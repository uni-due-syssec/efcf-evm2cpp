pragma solidity ^0.7;

contract SuicidalWithCondition {

  address payable owner; 
  mapping(address => uint256) deposits;
  uint256 raised = 0;

  constructor() {
    owner = msg.sender;
    raised = 0;
  }

  function invest() public payable {
    deposits[msg.sender] += msg.value;  
    raised += msg.value; 
  }

  function destroy() public {
    require(raised > 0);
    selfdestruct(msg.sender);
  }

}
