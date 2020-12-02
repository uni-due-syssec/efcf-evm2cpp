# evm2cpp - Compile EVM Bytecode to C++ Code

`evm2cpp` is a compiler for EVM instructions into semantically equivalent C++
code. `evm2cpp` is not a decompiler. The generated C++ code is mostly
unreadable. `evm2cpp` treats C++ as a code generation target. The generated
C++ code is intended to be paired with a EVM implementation that implements the
opcode handlers as required by `evm2cpp` (only the modified eEVM from the EF/CF
projected is supported).

The idea behind this tool was basically the first Futamura projection: We take
an EVM interpreter and a concrete EVM program and create a specialized
interpreter that only runs this one program. From a code-generation point of
view, we use a quite a simple approach: for each opcode, we emit a call to the
respective opcode handler. This allows us to eliminate the interpreter loop (a
similar approach to the
[sparkplug jit-compiler of chrome](https://v8.dev/blog/sparkplug)).

Interestingly the EVM has one nice feature, which is pretty nice for code
generation. In EVM every potential jump destination is marked with a special
marker: the `JUMPDEST` pseudo-instruction. This is in fact a no-op instruction.
However, due to this, we can essentially determine all basic block boundaries
with a single linear pass over the code. We translate each EVM basic block to a
C++ lexical block. This then allows us to utilize `goto` statements to jump
between the EVM basic blocks. To drastically simplify the compiler we restrict
optimizations to a single basic block. While this misses some optimization
opportunities, it also avoids any error-prone analysis (e.g., generating a
precise CFG from the EVM bytecode).


*Why do all this though?*  

* High execution speed
* Re-use C++ compiler optimizations
* Utilize standard C/C++ fuzzer tooling
* High-speed instrumentation

## EVM to CPP Transpiler

To translate the `Crowdsale` contract, we can run `evm2cpp` the following way

```
cargo run crowdsale ./contracts/Crowdsale.bin-runtime
```

However, the recommended way to run `evm2cpp` is to utilize a combined json
ouptut of the solidity compiler as input for `evm2cpp`, e.g.,

```
cargo run crowdsale ./contracts/crowdsale.combined.json
```

(Have a look at the `./contracts/Makefile` on how to generate this. This will
also automatically write the contract ABI definition into the
`./eEVM/fuzz/abi/` directory, which is highly recommended for fuzzing)

Currently the `evm2cpp` only translates the runtime part of a smart contract.
We assume that the constructor is only used once, so it does not benefit of the
C++ translation speedup: it must be run with the general interpreter instead.
If `evm2cpp` can identify the constructor bytecode, it will write it also to
the respective generated `.cpp` file for easy access.

Additionally, we can also add source code mapping information to the generated
C++ code. This is primarily useful for debugging the code generation or
debugging the contract (this is done automatically for combined json input).

```
cargo run crowdsale ./contracts/Crowdsale.bin-runtime ./contracts/Crowdsale.bin ./contracts/Crowdsale.sourcemap ./contracts/crowdsale.sol
```

The sourcemap is a bit tricky to generate. We need to utilize the combined json
output of the Solidity compiler.

```sh
solc --overwrite -o ./contracts \
    --combined-json srcmap-runtime \
    ./contracts/crowdsale.sol 
cat ./contracts/combined.json \
    | jq -r '.contracts["./contracts/crowdsale.sol:Crowdsale"]["srcmap-runtime"]' \
    > ./contracts/Crowdsale.sourcemap
```

## CLI Options

See `--help`

```
EVM bytecode to C++ transpiler targeting the eEVM framework

USAGE:
    evm2cpp [FLAGS] [OPTIONS] <name> [ARGS]

FLAGS:
    -F, --clang-format            launch clang-format on generated code
    -c, --combined-json           force use of combined json as input (auto-detected on filetype)
    -s, --emit-sourcemap          emit source information to generated code for easier codegen debugging
    -h, --help                    Prints help information
    -C, --single-combined-json    force use of combined json of a single contract (i.e., truffle-style)
    -A, --translate-all           Translate all contracts found in combined.json
    -V, --version                 Prints version information

OPTIONS:
    -a, --abi <ABI_FILE>                 path to abi definition file
        --contract-name <NAME>           contract name to look for in the combined.json input format (defaults to the
                                         <name> parameter)
    -e, --evm-path <EVM_PATH=./eEVM/>    path to eEVM project

ARGS:
    <name>                name/identifier of the contract for the generated code
    <input>               path to EVM runtime code (.bin-runtime) or combined-json input
    <constructor_path>    path to EVM constructor code (.bin)
```


## Knowns Issues

* Sourcemap parsing is somewhat wonky and mapping to source code is sometimes
  not very helpful, but really this only affects you if you want to change and
  debug the code generation.
