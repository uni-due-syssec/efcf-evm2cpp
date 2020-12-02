// https://github.com/crytic/echidna/blob/4a4518b89a90f6663f39a1feb48c31fda76235cb/examples/solidity/exercises/simple.sol#L1
pragma solidity ^0.4.16;

contract EchidnaBoomSuicidal {
  uint private counter=2**200;

  function inc(uint val) returns (uint){
    uint tmp = counter;
    counter += val;
    if (tmp > counter) {
        selfdestruct(0);
    } else {
        return (counter - tmp);
    }
  }

  function boom() returns (bool){
    return(true);
  }
}
