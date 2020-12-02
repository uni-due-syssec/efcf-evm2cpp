// Example taken from teEther repo: https://github.com/nescio007/teether/
// adapted to Solidity 0.7.6

pragma solidity ^0.7.6;

contract TeetherTestIf {

    struct Transaction{
        address payable to;
        uint amount;
    }

    mapping (bytes32 => Transaction) transactions;

    address owner;
    
    function set_owner(address new_owner) public {
        owner = new_owner;
    }
    
    function new_transaction(address payable to, uint amount) public returns (bytes32) {
        bytes32 token = keccak256(abi.encodePacked(to, amount));
        Transaction storage t = transactions[token];
        t.to = to;
        t.amount += amount;
        return token;
    }
    
    function approve(bytes32 token) public {
        require(owner == msg.sender);
        Transaction storage t = transactions[token];
        if (t.to != address(0)) {
          t.to.transfer(t.amount);
        }
        delete transactions[token];
    }
}
