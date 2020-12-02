// Copyright 2021 Michael Rodler
// This file is part of evm2cpp.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use crate::instructions::Instruction;
use bitvec::prelude::*;
use ethereum_types::U256;

const U256_ZERO: U256 = U256::zero();
const U256_ONE: U256 = U256([1, 0, 0, 0]);

/// Mapping of valid jump destination from code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeMeta {
    jumpdests: BitVec,
    iscode: BitVec,
}

#[allow(dead_code)]
impl CodeMeta {
    /// Create a new jumpdest/data mapping from given code bytes.
    pub fn new(code: &[u8]) -> Self {
        let mut jumpdests = BitVec::with_capacity(code.len());
        jumpdests.resize(code.len(), false);
        let mut iscode = BitVec::with_capacity(code.len());
        iscode.resize(code.len(), true);

        let mut i = 0;
        while i < code.len() {
            match Instruction::from_u8(code[i]) {
                Some(Instruction::JUMPDEST) => {
                    jumpdests.set(i, true);
                    i += 1;
                }
                Some(inst) => {
                    // PUSH is the only instruction, which has data encoded into the bytecode
                    if let Some(v) = inst.push_bytes() {
                        // we need to make sure we do not go out of bounds of the code in case there
                        // is accidentally a push instruction at the very end of the contract.
                        let lb = std::cmp::min(i + 1, code.len());
                        let up = std::cmp::min(i + 1 + v as usize, code.len());
                        // we set the push bytes explicitly to be data bytes
                        // note that the JUMPDEST instruction byte within a PUSH instruction is NOT
                        // a valid jump destination. So we should exclude those.
                        for j in lb..up {
                            iscode.set(j, false);
                        }
                        i += v as usize + 1;
                    } else {
                        // all other instructions are one byte
                        i += 1;
                    }
                }
                None => {
                    // invalid instruction; However, we do not mark it as data in EVM any
                    // instruction can be used as "INVALID". Early smart contracts simply used an
                    // invalid instruction to "throw an exception" and revert the internal
                    // transaction. (nowadays REVERT is used, which supports a return value)
                    i += 1;
                }
            }
        }

        CodeMeta { jumpdests, iscode }
    }

    /// Get the length of the mapping. This is the same as the
    /// code bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.jumpdests.len()
    }

    /// Returns true if the code bytes was also empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if the position is a valid jump destination.
    pub fn is_valid_jumpdest(&self, position: usize) -> bool {
        if position >= self.jumpdests.len() {
            return false;
        }
        self.jumpdests[position]
    }

    /// Returns `true` if the position is a valid instruction and not a byte that is part of a push
    /// instruction.
    pub fn is_instruction(&self, position: usize) -> bool {
        if position >= self.iscode.len() {
            return false;
        }
        self.iscode[position]
    }
}

/// A unique identifier within a basic block
pub type IInstRef = usize;

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Operand {
    /// Reference to the EVM Stack before the instruction with the given IInstRef and the stack/args
    /// offset
    StackRef((IInstRef, usize)),
    /// A stack pop at the instruction; only used in non-optimized IR
    StackPop((IInstRef, usize)),
    /// A constant value and a reference to the instruction that produced the constant; This is
    /// primarily used during constant folding/propagation
    Constant((IInstRef, U256)),
    /// Value returned by an instruction, with ID to reference the instruction and the offset on the stack of the returned value; This is primarily used to model data-flow information between instructions.
    InstructionRef((IInstRef, usize)),
}

/// "Intermediate Instruction" - this is the main instruction structure for our analysis. It is
/// primarily a wrapper around the `Instruction` struct with additional metadata attached to it.
#[derive(Clone, Debug)]
pub struct IInstruction {
    /// the PC/address of the instruction
    pub address: usize,
    /// given a global list of instructions, this would be the index. this is different from the
    /// address, because we
    pub global_idx: usize,
    /// Either a Instruction struct or the raw byte if it was an invalid byte
    pub opcode: Result<Instruction, u8>,
    /// A list of operands that the instruction takes; unoptimized this is only StackPop's and after
    /// optimization this can be Constants, StackRef's and InstructionRef's
    pub operands: Option<Vec<Operand>>,
    //results: Option<Vec<Operand>>,
    /// whether the instruction returns a constant value. note that this must not necessarily be a
    /// Operand::Constant, but can also be a Operand::InstructionRef or Operand::StackRef; the
    /// important part is that the returned value is not changed by the current instruction.
    pub is_constant: bool,
    /// useful for codegen later on; this instruction can be ignored (i.e., it does not perform
    /// meaningful computation; e.g., DUPx, SWAPx, ADD of two constant integers, etc.)
    pub ignoreable: bool,
    /// if the Intruction returns a Operand::Constant, then the values are stored here.
    pub value: Option<Vec<U256>>,
}

impl IInstruction {
    pub fn get_name(&self) -> String {
        if let Ok(op) = self.opcode {
            op.info().name.to_ascii_lowercase()
        } else {
            "invalid".to_string()
        }
    }
}

/// We can represent one Basic Block as a logical unit in terms of stack operations. Within one basic
/// block we can then transform all stack operations to abstract data-flows, which we can lower to
/// normal register-based (or C++ variables in the case of the cpp backend).
///
/// Let's consider the following basic block, which implements a increment by one as a internal
/// function / subroutine. Calling convention is that the arguments are pushed on the stack, then the
/// return address. The result is returned instead of the return address. arguments must be cleaned
/// by the caller.
///
/// ```
/// ...
/// DUP2
/// PUSH 0x01
/// ADD
/// SWAP1
/// JUMP
/// ...
/// ```
///
/// We will translate this to a BasicBlock summary, which contains the following information:
///
/// ```
/// BBSummary(
///     args=[
///         Ref(1), /// from the dup2
///         Pop(), /// from the jump
///     ],
///     ret=[
///         Add(Ref(1), Constant(1)),
///     ]
/// )
/// ```
///
/// This allows us to generate one lexical C++ block per EVM Basic Block, while still keeping the
/// overall stack effects of a single EVM basic block the same. The latter is important since we
/// translate each basic block indepenently. Keeping the basic blocks independent and using the EVM
/// stack as before to pass data between BBs, allows us to avoid "inter procedural" data-flow
/// analysis between EVM basic blocks. We don't do this, because we can avoid all kinds of funky
/// things needed for such an analysis between basic blocks. Most notably, we would need a more
/// sophisticated abstract interpretation or symbolic analysis to identify the transitions between
/// basic blocks.
///
/// The example above can be code-gen'ed to C++ code that looks something like this:
///
/// ```c++
/// {
///   // BB arguments (first refs, then pops)
///   auto arg_ref_1 = ctxt->s.peek(1);
///   auto arg_pop_0 = ctxt->s.pop();
///   // BB computation
///   auto r0 = add(arg_ref_0, 0x01);
///   // push end
///   ctxt->s.push(r0);
///   JUMP(arg_pop_0);
/// }
/// ```
///
/// In practice th BasicBlock structure looks a bit different, but basically has the same
/// information.
#[derive(Clone, Debug)]
pub struct BasicBlock {
    /// address of the first instruction in the BasicBlock
    pub address: usize,
    /// List of instructions in the basic block
    pub instructions: Vec<IInstruction>,
    /// If the stack is bigger after the BB, these are the "return values" of the BB; they can be
    /// pushed on the stack at the end onto the evm stack
    pub returns: Vec<Operand>,
    /// These are the stack-set operations (due to e.g., SWAP). The basic block write to the
    /// following stack offsets. This is a mapping from stack offset to Operand.
    pub stack_sets: std::collections::BTreeMap<usize, Operand>,
    /// If the stack is smaller after the BB, this is the number of stack slots that need to be
    /// popped at the end of the basic block.
    pub pops_at_end: usize,
    /// currently we can only run the optimizer once, so we need to avoid breaking everything on
    /// a second call to optimize(); this is false before a call to optimize() and true afterwards;
    /// subsequent optimize() calls will just be a nop.
    optimized: bool,
    /// If the BasicBlock is terminated with an invalid instruction.
    pub ends_on_invalid: bool,
}

impl BasicBlock {
    fn parse(bytecode: &[u8], start_index: usize, inst_global_index: usize) -> (BasicBlock, usize) {
        let mut inst_global_index = inst_global_index;
        // Build Basic Block
        let mut bb = BasicBlock {
            address: start_index,
            instructions: vec![],
            returns: vec![],
            stack_sets: std::collections::BTreeMap::new(),
            pops_at_end: 0,
            optimized: false,
            ends_on_invalid: false,
        };
        let mut idx = 0;
        let mut pc = start_index;
        while pc < bytecode.len() {
            let mut iinst = IInstruction {
                address: pc,
                global_idx: inst_global_index,
                opcode: Err(bytecode[pc]),
                is_constant: false,
                operands: None,
                value: None,
                ignoreable: false,
            };
            inst_global_index += 1;
            let mut should_break = false;
            if let Some(inst) = Instruction::from_u8(bytecode[pc]) {
                let push_bytes_sz = inst.push_bytes().unwrap_or(0);
                let next_pc = pc + 1 + push_bytes_sz;
                // we also need to end the BB in case the next instruction is a JUMPDEST
                let next_is_jumpdest = if next_pc < bytecode.len() {
                    if let Some(jdest) = Instruction::from_u8(bytecode[next_pc]) {
                        jdest == Instruction::JUMPDEST
                    } else {
                        false
                    }
                } else {
                    false
                };
                if inst.stops() || inst.is_jump() || next_is_jumpdest {
                    should_break = true;
                }
                iinst.is_constant = inst.pushes_constant();
                let info = inst.info();
                // For instructions that push constants, we store the constant
                // value here
                if inst.pushes_constant() {
                    if let Some(n) = inst.push_bytes() {
                        let hex_bytes = if (pc + 1 + n) < bytecode.len() {
                            U256::from_big_endian(&bytecode[pc + 1..pc + 1 + n])
                        } else {
                            if pc + 1 == bytecode.len() {
                                U256::from(0)
                            } else {
                                U256::from_big_endian(&bytecode[pc + 1..bytecode.len()])
                            }
                        };
                        iinst.value = Some(vec![hex_bytes]);
                    } else if inst == Instruction::PC {
                        iinst.value = Some(vec![U256::from(pc)]);
                    } else {
                        panic!("Cannot handle instruction of type {:?} at pc {:#x}, which is supposed to push a constant", inst, pc);
                    }
                }
                // We treat codesize() calls as a constant, since we could assume that
                // we know our own codesize. However, these calls are not actually always constant
                // since we could've been called as via DELEGATECALL and then we would a get a
                // different codesize. Not sure whether this is used a lot, though?
                if inst == Instruction::CODESIZE {
                    iinst.value = Some(vec![U256::from(bytecode.len())]);
                }

                // next we handle the operands. Essentially we have three different
                // cases: DUP and SWAP, which can access stuff on the stack and all other
                // instructions, which simply pop their arguments off the stack.
                iinst.operands = if let Some(pos) = inst.dup_position() {
                    Some(vec![Operand::StackRef((idx, pos))])
                } else if let Some(pos) = inst.swap_position() {
                    Some(vec![
                        Operand::StackRef((idx, 0)),
                        Operand::StackRef((idx, pos)),
                    ])
                } else {
                    if inst.info().args == 0 {
                        None
                    } else {
                        Some(
                            (0..info.args)
                                .map(|i| Operand::StackPop((idx, i)))
                                .collect(),
                        )
                    }
                };

                // advance to next instruction
                if let Some(n) = inst.push_bytes() {
                    // skip push bytes
                    pc += n;
                }
                iinst.opcode = Ok(inst);
            } else {
                bb.ends_on_invalid = true;
                should_break = true;
            }

            bb.instructions.push(iinst);

            // advance to next instruction
            pc += 1;
            idx += 1;

            if should_break {
                break;
            }
        }

        (bb, pc)
    }

    /// Emulate the Basic Block on a abstract stack to transform Stack Machine Instructions
    /// to IR for code-generation of register-based instructions. Performs constant propagation
    /// along the way.
    ///
    /// WARNING: never call twice on the same BasicBlock, as this method assumes a unoptimized
    /// state.
    ///
    /// for example, the pops_at_end field is somewhat dual-purposed, it must be 0 at start and it
    /// is increased during emulation to keep track of the correct stack offsets.
    fn emulate_bb(&mut self) -> Option<Vec<Operand>> {
        // emulated evm stack with abstract values
        let mut evm_stack = std::collections::VecDeque::<Operand>::with_capacity(128);

        // We populate the abstract stack with unknown stack reference placeholder values, such
        // that stack emulation can operate also on unknown values. We still have to special case
        // for when a basic block accesses more than 32 values, but that should be sufficiently
        // rare.
        for i in 0..32 {
            evm_stack.push_back(Operand::StackRef((0, i)));
        }
        let evm_stack_initial_len = evm_stack.len();

        // now we loop through the instructions and emulate them
        for idx in 0..self.instructions.len() {
            let mut inst = self.instructions[idx].clone();

            if let Ok(evm_inst) = inst.opcode {
                let evm_iinfo = evm_inst.info();

                // stack operation. again we need to handle the following cases:
                // * PUSH / PC (pushes constant)
                // * DUP
                // * SWAP
                // * POP
                // * All Other Instructions
                if evm_inst.pushes_constant() {
                    /*******************/
                    // PUSH / PC
                    /*******************/

                    // This always pushes a constant value.
                    inst.ignoreable = true;
                    let value_vec = inst.value.clone().unwrap();
                    let v = value_vec[0];
                    evm_stack.push_front(Operand::Constant((idx, v)));
                } else if let Some(pos) = evm_inst.dup_position() {
                    /*******************/
                    // DUP
                    /*******************/

                    // we set dups to be always ignoreable during codegen
                    inst.ignoreable = true;

                    let stack_len = evm_stack.len();

                    if pos < stack_len {
                        // we can dup inside of the emulated stack
                        let x = evm_stack[pos];

                        // transform the DUP to a constant
                        if let Operand::Constant((_, v)) = x {
                            inst.is_constant = true;
                            inst.value = Some(vec![v]);
                        }
                        // store operand in instruction
                        inst.operands = Some(vec![x.clone()]);
                        // push the constant back to stack;
                        evm_stack.push_front(x);
                    } else {
                        // we are duping something completely unknown.

                        // the operand is transformed to something new
                        let pos_at_bb_start =
                            pos - stack_len + evm_stack_initial_len + self.pops_at_end;
                        let a = Operand::StackRef((0, pos_at_bb_start));
                        inst.operands = Some(vec![a]);

                        evm_stack.push_front(a);
                    }
                } else if let Some(pos) = evm_inst.swap_position() {
                    /*******************/
                    // SWAP
                    /*******************/

                    let stack_len = evm_stack.len();

                    if stack_len == 0 {
                        // Two reference parameters
                        inst.ignoreable = false;
                        inst.operands = Some(vec![
                            Operand::StackRef((0, 0 + self.pops_at_end + evm_stack_initial_len)),
                            Operand::StackRef((0, pos + self.pops_at_end + evm_stack_initial_len)),
                        ]);
                    // emulated stack is not affected
                    } else if stack_len > 0 && pos >= stack_len {
                        // there is a value on the top of the emulated stack.
                        // now this one is tricky, since we need to swap a known operand with an
                        // unknown value.
                        inst.ignoreable = false;
                        let pos_at_bb_start =
                            pos - stack_len + evm_stack_initial_len + self.pops_at_end;
                        inst.operands = Some(vec![
                            evm_stack.pop_front().unwrap(),
                            Operand::StackRef((0, pos_at_bb_start)),
                        ]);

                        evm_stack.push_front(Operand::StackRef((0, pos_at_bb_start)));
                    } else {
                        // both are on the emulated stack
                        inst.ignoreable = true;
                        inst.operands = Some(vec![evm_stack[0].clone(), evm_stack[pos].clone()]);
                        evm_stack.swap(0, pos);
                    }
                } else if evm_inst == Instruction::POP {
                    /*********************/
                    // POP
                    /*********************/

                    // pops can be ignored for sure
                    inst.ignoreable = true;
                    if evm_stack.len() > 0 {
                        // either we perform an emulated pop
                        evm_stack.pop_front();
                    } else {
                        // or we instruct the BB to pop the value the end
                        self.pops_at_end += 1;
                    }
                    // we remove the operands
                    inst.operands = None;
                } else if evm_inst == Instruction::JUMPDEST {
                    inst.ignoreable = true;
                } else {
                    /***********************/
                    // non-stack related ops
                    /***********************/

                    let mut args = Vec::<Operand>::new();
                    let cur_pops_at_end = self.pops_at_end;
                    let cur_stack_len = evm_stack.len();
                    for args_idx in 0..evm_iinfo.args {
                        if let Some(val) = evm_stack.pop_front() {
                            args.push(val);
                        } else {
                            // referring to a stack slot that was not pushed
                            // within the current BB
                            let pos_at_bb_start =
                                args_idx + cur_pops_at_end - cur_stack_len + evm_stack_initial_len;
                            args.push(Operand::StackRef((0, pos_at_bb_start)));
                            self.pops_at_end += 1;
                        }
                    }
                    let (vvec, evals_to_constant) = evaluate_opcode(evm_inst, idx, &args);
                    if evals_to_constant {
                        inst.is_constant = true;
                        inst.value = Some(
                            vvec.iter()
                                .filter_map(|x| {
                                    if let Operand::Constant((_, v)) = *x {
                                        Some(v)
                                    } else {
                                        None
                                    }
                                })
                                .collect(),
                        );
                        inst.ignoreable = true;
                    }
                    for v in vvec.into_iter() {
                        //if let Operand::StackRef((idx, stack_offset)) = v {
                        //}
                        evm_stack.push_front(v);
                    }

                    if args.len() > 0 {
                        inst.operands = Some(args);
                    }
                }
            } else {
                // not a real instruction, but raw data, we stop optimization then.
                // we only want to optimize "well formed" basic blocks
                return None;
            }

            self.instructions[idx] = inst;
        }

        // end of the basic block, now we have to make sure the stack is consistent, i.e., if we
        // would execute all stack operations then the stack should look the same at the end of the
        // basic block.
        // 2. checking for excess pushes/pops
        let mut evm_stack = evm_stack.into_iter();
        let mut offset = 0;
        let returns = if evm_stack.len() > evm_stack_initial_len {
            // BB pushes more than pops -> we need to push the remaining values to the stack at the
            // end of the basic block.
            let new_stack_slots_count = evm_stack.len() - evm_stack_initial_len;
            let r: Vec<Operand> = evm_stack.by_ref().take(new_stack_slots_count).collect();
            //for (idx, stack_item) in s.enumerate() {
            //    if stack_item != Operand::StackRef((0, idx)) {
            //        self.stack_sets.insert(idx, stack_item);
            //    }
            //}
            Some(r)
        } else if evm_stack.len() < evm_stack_initial_len {
            // BB pops more than pushes -> we need to pop the unneeded values from the stack
            offset = evm_stack_initial_len - evm_stack.len();
            self.pops_at_end += offset;
            None
        } else {
            // BB pushes and pops are equal -> no effect on stack
            None
        };

        for (idx, stack_item) in evm_stack.into_iter().enumerate() {
            // we compute the real idx here, which is the stack index + the pops at the end
            let idx = idx + offset;
            if stack_item != Operand::StackRef((0, idx)) {
                self.stack_sets.insert(idx, stack_item);
            }
        }

        returns
    }

    //fn mark_ignorable(&mut self) {}

    pub fn optimize(&mut self) {
        if self.optimized {
            return;
        }
        self.optimized = true;

        if let Some(stack_remainder) = self.emulate_bb() {
            self.returns = stack_remainder.into_iter().collect();
        }
    }
}

/// implement constant folding if applicable to instruction
fn evaluate_opcode(evm_inst: Instruction, idx: usize, args: &Vec<Operand>) -> (Vec<Operand>, bool) {
    let mut ret = Vec::<Operand>::new();
    let evm_iinfo = evm_inst.info();
    let mut is_constant = false;
    #[cfg(debug_assertions)]
    if evm_iinfo.args != args.len() {
        panic!(
            "args vec length != expected for given opcode {:?}; args vec was {:?}",
            evm_inst, args
        );
    }

    // const_eval_result should be set to None, when the provided args cannot be evaluated to a
    // constant Operand for the given opcode; if the args can be evaluated to a constant Operand,
    // then do it and return the newly created Operand. Note that the instruction will be marked as
    // constant then and will be omitted during code generation later on. If any of the instructions
    // do have side-effects outside of the evm stack, then this variable must be set to None.
    // Note that the Operand can be of any type (e.g.., also Operand::InstructionRef) and not only
    // Operand:Constant (although this is the major case).
    let const_eval_result: Option<Operand> = if args.len() == 1 {
        if let Operand::Constant((_, a)) = args[0] {
            match evm_inst {
                Instruction::ISZERO => Some(Operand::Constant((
                    idx,
                    if a.is_zero() {
                        U256::one()
                    } else {
                        U256::zero()
                    },
                ))),
                Instruction::NOT => Some(Operand::Constant((idx, !a))),
                _ => None,
            }
        } else {
            None
        }
    } else if args.len() == 2 {
        if let (Operand::Constant((_, a)), Operand::Constant((_, b))) = (args[0], args[1]) {
            match evm_inst {
                Instruction::ADD => Some(Operand::Constant((idx, a.overflowing_add(b).0))),
                Instruction::MUL => Some(Operand::Constant((idx, a.overflowing_mul(b).0))),
                Instruction::SUB => Some(Operand::Constant((idx, a.overflowing_sub(b).0))),
                Instruction::DIV => {
                    if b.is_zero() {
                        Some(Operand::Constant((idx, U256::zero())))
                    } else {
                        Some(Operand::Constant((idx, a / b)))
                    }
                }
                // missing inst: SDIV
                Instruction::MOD => {
                    if b.is_zero() {
                        Some(Operand::Constant((idx, U256::zero())))
                    } else {
                        Some(Operand::Constant((idx, a % b)))
                    }
                }
                // missing inst: SMOD
                Instruction::EXP => Some(Operand::Constant((idx, a.overflowing_pow(b).0))),
                // missing inst: SIGNEXTEND
                Instruction::LT => Some(Operand::Constant((
                    idx,
                    if a < b { U256::one() } else { U256::zero() },
                ))),
                Instruction::GT => Some(Operand::Constant((
                    idx,
                    if a > b { U256::one() } else { U256::zero() },
                ))),
                // missing inst: SLT
                // missing inst: SGT
                Instruction::EQ => Some(Operand::Constant((
                    idx,
                    if a == b { U256::one() } else { U256::zero() },
                ))),
                Instruction::AND => Some(Operand::Constant((idx, a & b))),
                Instruction::OR => Some(Operand::Constant((idx, a | b))),
                Instruction::XOR => Some(Operand::Constant((idx, a ^ b))),
                Instruction::BYTE => {
                    if a < U256::from(32) {
                        let byte = (b >> (8 * (31 - a.low_u64() as usize))) & U256::from(0xff);
                        Some(Operand::Constant((idx, byte)))
                    } else {
                        Some(Operand::Constant((idx, U256::zero())))
                    }
                }
                Instruction::SHR => {
                    if a <= U256::from(usize::MAX) {
                        Some(Operand::Constant((idx, b >> a)))
                    } else {
                        // shifted into oblivion... so zero
                        Some(Operand::Constant((idx, U256_ZERO)))
                    }
                }
                Instruction::SHL => {
                    if a <= U256::from(usize::MAX) {
                        Some(Operand::Constant((idx, b << a)))
                    } else {
                        Some(Operand::Constant((idx, U256_ZERO)))
                    }
                }
                // missing inst: SAR
                _ => None,
            }
        } else {
            // one of the operands is not a constant, but we can still do some calculations when
            // identitiy values for the respective instruction are involved
            if evm_inst == Instruction::ADD {
                match (args[0], args[1]) {
                    // special case for additive identities
                    // for all i: i + 0 == i
                    (x, Operand::Constant((_, U256_ZERO))) => Some(x),
                    // for all i: 0 + i == i
                    (Operand::Constant((_, U256_ZERO)), x) => Some(x),
                    _ => None,
                }
            } else if evm_inst == Instruction::SUB {
                match (args[0], args[1]) {
                    // special case for subtractive identities
                    // for all i: i - 0 == i
                    (x, Operand::Constant((_, U256_ZERO))) => Some(x),
                    _ => None,
                }
            } else if evm_inst == Instruction::MUL {
                match (args[0], args[1]) {
                    // special case for multiplicative identities
                    // for all i; i * 1 == i
                    (x, Operand::Constant((_, U256_ONE))) => Some(x),
                    // for all i; 1 * i == i
                    (Operand::Constant((_, U256_ONE)), x) => Some(x),
                    // for all i; i * 0 == 0
                    (_, Operand::Constant((_, U256_ZERO))) => {
                        Some(Operand::Constant((idx, U256_ZERO)))
                    }
                    // for all i; 0 * i == 0
                    (Operand::Constant((_, U256_ZERO)), _) => {
                        Some(Operand::Constant((idx, U256_ZERO)))
                    }
                    _ => None,
                }
            } else if evm_inst == Instruction::DIV {
                match (args[0], args[1]) {
                    // for all i: i / 0 == 0 (in the EVM)
                    (_, Operand::Constant((_, U256_ZERO))) => {
                        Some(Operand::Constant((idx, U256_ZERO)))
                    }
                    // for all i: i / 1 == i
                    (x, Operand::Constant((_, U256_ONE))) => Some(x),
                    // for all i: 0 / i == 0
                    (Operand::Constant((_, U256_ZERO)), _) => {
                        Some(Operand::Constant((idx, U256_ZERO)))
                    }
                    _ => None,
                }
            } else if evm_inst == Instruction::EXP {
                match (args[0], args[1]) {
                    // for all i: i ** 0 == 1
                    (_, Operand::Constant((_, U256_ZERO))) => {
                        Some(Operand::Constant((idx, U256_ONE)))
                    }
                    // for all i: i ** 1 == i
                    (x, Operand::Constant((_, U256_ONE))) => Some(x),
                    // for all i: 0 ** i == 0
                    (Operand::Constant((_, U256_ZERO)), _) => {
                        Some(Operand::Constant((idx, U256_ZERO)))
                    }
                    _ => None,
                }
            } else if evm_inst == Instruction::SHR || evm_inst == Instruction::SHL {
                match (args[0], args[1]) {
                    // for all i: i >> 0 == i
                    // for all i: i << 0 == i
                    (Operand::Constant((_, U256_ZERO)), arg1) => Some(arg1),
                    _ => None,
                }
            } else {
                None
            }
        }
    } else if args.len() == 3 {
        if let (Operand::Constant((_, _a)), Operand::Constant((_, _b)), Operand::Constant((_, c))) =
            (args[0], args[1], args[2])
        {
            match evm_inst {
                Instruction::ADDMOD => {
                    if !c.is_zero() {
                        // TODO: not clear if this is a correct implementation for the ADD/MULMOD
                        // instructions.
                        //```
                        //Some(Operand::Constant((idx, a.overflowing_add(b).0 % c)));
                        //```
                        // Do we need to propagate to a bigger type? The parity EVM converts to a
                        // BigUint first before doing the add/modulo. Not sure why though.
                        // https://github.com/openethereum/openethereum/blob/15b5581894d6f9e1a51ed34ffc5497301a36dacb/ethcore/evm/src/interpreter/mod.rs#L1362
                        // TODO: do we even need those instructions? they seem sufficiently rare.
                        // TODO: can we handle some other special cases, (i.e., a or b is 0)
                        //
                        // WORKAROUND: for now, we just bail out and don't do any constant propagation
                        None
                    } else {
                        Some(Operand::Constant((idx, U256::zero())))
                    }
                }
                Instruction::MULMOD => {
                    if !c.is_zero() {
                        //Some(Operand::Constant((idx, a.overflowing_mul(b).0 % c)));
                        None
                    } else {
                        Some(Operand::Constant((idx, U256::zero())))
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some(res) = const_eval_result {
        ret.push(res);
        is_constant = true;
    }
    debug_assert!(ret.len() <= evm_iinfo.ret);

    if ret.len() < evm_iinfo.ret {
        is_constant = false;
        for ret_idx in ret.len()..evm_iinfo.ret {
            ret.push(Operand::InstructionRef((idx, ret_idx)));
        }
    }

    debug_assert!(ret.len() == evm_iinfo.ret);

    (ret, is_constant)
}

#[derive(Clone, Debug)]
pub struct Program {
    pub bytecode: Vec<u8>,
    pub basic_blocks: Vec<BasicBlock>,
    pub meta: CodeMeta,
}

impl Program {
    pub fn new(bytecode: &[u8]) -> Program {
        let mut blocks = Vec::<BasicBlock>::new();
        let mut index = 0;
        let mut inst_global_index = 0;
        while index < bytecode.len() {
            let (bb, pc) = BasicBlock::parse(bytecode, index, inst_global_index);
            inst_global_index += bb.instructions.len();
            blocks.push(bb);
            index = pc;
        }
        Program {
            bytecode: Vec::from(bytecode),
            meta: CodeMeta::new(bytecode),
            basic_blocks: blocks,
        }
    }

    pub fn optimize(&mut self) {
        for bb in self.basic_blocks.iter_mut() {
            bb.optimize();
        }
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn u256_const_equalities() {
        assert_eq!(U256_ONE, U256::from(1));
        assert!(!U256_ONE.is_zero());
        assert_eq!(U256_ZERO, U256::from(0));
        assert!(U256_ZERO.is_zero());
    }

    #[test]
    fn build_program() {
        /*
         * 0: PUSH1 0x4 [60 04];
         * 2: JUMP [56]; branches to loc_4
         * 3: STOP [00];
         * 4: JUMPDEST [5b]; loc_4
         * 5: PUSH1 0x4 [60 04];
         * 7: JUMP [56]; branches to loc_4
         */
        let bytecode_str = "0x600456005b600456";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let mut program = Program::new(&bytecode);

        println!("program: {:?}", program);
        //let (mut bb, pc) = BasicBlock::parse(&bytecode, 0);
        program.optimize();
        println!("program': {:?}", program);

        assert_eq!(program.basic_blocks.len(), 3);
    }

    #[test]
    fn push_constant_prop() {
        // 0: PUSH1 0xff [60 ff];
        // 2: JUMP [56];
        let bytecode_str = "0x60ff56";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("bytecode: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 2);
        assert_eq!(pc, 3);

        // start optimizer
        bb.optimize();
        let bb = bb; // get rid of mut?

        // check results
        let push_inst = bb.instructions[0].clone();
        assert_eq!(push_inst.opcode, Ok(Instruction::PUSH1));
        let jump_inst = bb.instructions[1].clone();
        assert_eq!(jump_inst.opcode, Ok(Instruction::JUMP));
        assert_eq!(
            jump_inst.operands.unwrap()[0],
            Operand::Constant((0, U256::from(0xff)))
        );
    }

    #[test]
    fn dup_constant_prop() {
        // 0: PUSH1 0xff [60 ff];
        // 2: DUP1 [80];
        // 3: JUMP [56];
        let bytecode_str = "0x60ff8056";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 3);
        assert_eq!(pc, 4);

        // start optimizer
        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        // check results
        let push_inst = bb.instructions[0].clone();
        assert_eq!(push_inst.opcode, Ok(Instruction::PUSH1));
        let jump_inst = bb.instructions[2].clone();
        assert_eq!(jump_inst.opcode, Ok(Instruction::JUMP));
        assert_eq!(
            jump_inst.operands.unwrap()[0],
            Operand::Constant((0, U256::from(0xff)))
        );
    }

    #[test]
    fn add_constant_prop() {
        // 0: PUSH1 0x01 [60 01];
        // 2: PUSH1 0x02 [60 02];
        // 4: ADD [01];
        // 5: JUMP [56]; illegal target
        let bytecode_str = "0x600160020156";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 4);
        assert_eq!(pc, 6);

        // start optimizer
        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        // check results
        let jump_inst = bb.instructions[3].clone();
        assert!(jump_inst.opcode == Ok(Instruction::JUMP));
        assert_eq!(
            jump_inst.operands.unwrap()[0],
            Operand::Constant((2, U256::from(3)))
        );
    }

    #[test]
    fn mstore_constant_prop() {
        // 0: PUSH1 0x80 [60 80];
        // 2: PUSH1 0x20 [60 20];
        // 4: MSTORE [52];
        // 5: JUMP [56]; illegal target
        let bytecode_str = "0x608060205256";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 4);
        assert_eq!(pc, 6);

        // start optimizer
        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        // check results
        let jump_inst = bb.instructions[3].clone();
        assert!(jump_inst.opcode == Ok(Instruction::JUMP));

        // mstore checks
        let inst = bb.instructions[2].clone();
        println!("checking {:?}", inst);
        assert_eq!(inst.ignoreable, false);
        assert!(inst.operands.is_some());
        let operands = inst.operands.unwrap();
        assert!(operands.len() == 2);
        assert_eq!(operands[0], Operand::Constant((1, U256::from(0x20))));
        assert_eq!(operands[1], Operand::Constant((0, U256::from(0x80))));
    }

    #[test]
    fn add_no_constant_prop() {
        // 2: PUSH1 0x02 [60 02];
        // 4: ADD [01];
        // 5: JUMP [56];
        let bytecode_str = "0x60020156";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 3);
        assert_eq!(pc, 4);

        // start optimizer
        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        // check results
        let jump_inst = bb.instructions[2].clone();
        assert!(jump_inst.opcode == Ok(Instruction::JUMP));
        assert_eq!(
            jump_inst.operands.unwrap()[0],
            Operand::InstructionRef((1, 0))
        );
    }

    #[test]
    fn no_constant_prop_possible() {
        // 0: DUP1 [80];
        // 1: JUMP [56];
        let bytecode_str = "0x8056";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 2);
        assert_eq!(pc, 2);

        // start optimizer
        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        assert_eq!(bb.instructions[0].ignoreable, true);
        assert_eq!(bb.instructions[1].ignoreable, false);

        // check results
        let jump_inst = bb.instructions[1].clone();
        assert!(jump_inst.opcode == Ok(Instruction::JUMP));
        assert_eq!(jump_inst.operands.unwrap()[0], Operand::StackRef((0, 0)));
    }

    #[test]
    fn bb_args_ret() {
        // 0: DUP1 [80];
        // 1: DUP1 [80];
        // 2: JUMP [56];
        let bytecode_str = "0x808056";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 3);
        assert_eq!(pc, 3);

        // start optimizer
        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        assert_eq!(bb.instructions[0].ignoreable, true);
        assert_eq!(bb.instructions[1].ignoreable, true);
        assert_eq!(bb.instructions[2].ignoreable, false);

        // check results
        let jump_inst = bb.instructions[2].clone();
        assert!(jump_inst.opcode == Ok(Instruction::JUMP));
        assert_eq!(jump_inst.operands.unwrap()[0], Operand::StackRef((0, 0)));
        let dup_inst = bb.instructions[0].clone();
        assert_eq!(dup_inst.operands.unwrap()[0], Operand::StackRef((0, 0)));
    }

    #[test]
    fn bb_args_add() {
        // 0: ADD [01];
        // 1: PUSH1 0x42 [60 42];
        // 3: JUMP [56]; illegal target
        let bytecode_str = "0x01604256";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 3);
        assert_eq!(pc, 4);

        // start optimizer
        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        // check results
        let jump_inst = bb.instructions[2].clone();
        assert!(jump_inst.opcode == Ok(Instruction::JUMP));

        // add checks
        let add_inst = bb.instructions[0].clone();
        println!("checking {:?}", add_inst);
        assert_eq!(add_inst.ignoreable, false);
        assert!(add_inst.operands.is_some());
        let add_operands = add_inst.operands.unwrap();
        assert!(add_operands.len() == 2);
        assert_eq!(add_operands[0], Operand::StackRef((0, 0)));
        assert_eq!(add_operands[1], Operand::StackRef((0, 1)));
    }

    #[test]
    fn bb_pop_unknown() {
        // 0: POP [50];
        // 1: STOP [00];
        let bytecode_str = "0x5000";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 2);
        assert_eq!(pc, 2);

        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        // check results
        let last_inst = bb.instructions[1].clone();
        println!("checking {:?}", last_inst);
        assert_eq!(last_inst.opcode, Ok(Instruction::STOP));
        assert!(last_inst.operands.is_none());
        assert_eq!(last_inst.ignoreable, false);
        // pop checks
        let pop_inst = bb.instructions[0].clone();
        println!("checking {:?}", pop_inst);
        assert!(pop_inst.operands.is_none());
        assert_eq!(bb.pops_at_end, 1);
    }

    #[test]
    fn bb_args_pop_add() {
        // 0: POP [50];
        // 1: ADD [01];
        // 2: STOP [00];
        let bytecode_str = "0x500100";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 3);
        assert_eq!(pc, 3);

        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        // check results
        assert_eq!(bb.pops_at_end, 2);
        let last_inst = bb.instructions[2].clone();
        println!("checking {:?}", last_inst);
        assert_eq!(last_inst.opcode, Ok(Instruction::STOP));
        assert!(last_inst.operands.is_none());
        assert_eq!(last_inst.ignoreable, false);
        // pop checks
        let pop_inst = bb.instructions[0].clone();
        println!("checking {:?}", pop_inst);
        assert_eq!(pop_inst.ignoreable, true);
        // add checks
        let add_inst = bb.instructions[1].clone();
        println!("checking {:?}", add_inst);
        assert_eq!(add_inst.ignoreable, false);
        assert!(add_inst.operands.is_some());
        let add_operands = add_inst.operands.unwrap();
        assert!(add_operands.len() == 2);
        assert_eq!(add_operands[0], Operand::StackRef((0, 1)));
        assert_eq!(add_operands[1], Operand::StackRef((0, 2)));
    }

    #[test]
    fn bb_args_pop_add_with_const() {
        // 0: POP [50];
        // 1: PUSH1 0x42 [60 42];
        // 3: ADD [01];
        // 4: STOP [00];
        let bytecode_str = "0x5060420100";
        let bytecode = hexutil::read_hex(bytecode_str).unwrap();
        let (mut bb, pc) = BasicBlock::parse(&bytecode, 0, 0);
        println!("BB: {:?}", bb.instructions);
        assert_eq!(bb.instructions.len(), 4);
        assert_eq!(pc, 5);

        bb.optimize();
        println!("BB': {:?}", bb.instructions);

        // check results
        assert_eq!(bb.pops_at_end, 1);
        let last_inst = bb.instructions[3].clone();
        println!("checking {:?}", last_inst);
        assert_eq!(last_inst.opcode, Ok(Instruction::STOP));
        assert!(last_inst.operands.is_none());
        assert_eq!(last_inst.ignoreable, false);
        // pop checks
        let pop_inst = bb.instructions[0].clone();
        println!("checking {:?}", pop_inst);
        assert_eq!(pop_inst.ignoreable, true);

        // add checks
        let add_inst = bb.instructions[2].clone();
        println!("checking {:?}", add_inst);
        assert_eq!(add_inst.ignoreable, false);
        assert!(add_inst.operands.is_some());
        let add_operands = add_inst.operands.unwrap();
        assert!(add_operands.len() == 2);
        assert_eq!(add_operands[0], Operand::Constant((1, U256::from(0x42))));
        assert_eq!(add_operands[1], Operand::StackRef((0, 1)));
    }
}
