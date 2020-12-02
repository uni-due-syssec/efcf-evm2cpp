pragma solidity ^0.7;

contract SuicidalWithMagicValueCondition {

  address payable owner; 
  mapping(address => uint256) deposits;
  uint256 raised = 0;
  uint256 key = 0;

  constructor(uint256 _key) {
    owner = msg.sender;
    raised = 0;
    key = _key;
  }

  function invest() public payable {
    deposits[msg.sender] += msg.value;  
    raised += msg.value; 
  }

  function destroy(uint256 provided_key) public {
    require(raised > 0);
    require(provided_key == key);
    selfdestruct(msg.sender);
  }

}
