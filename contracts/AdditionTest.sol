pragma solidity >=0.7.0 <0.9.0;
contract Addition {
    function add(uint256 num1, uint256 num2) public pure returns (uint256){}
}
contract AdditionTest {
    Addition a;
    uint additionResult = 0;

    function add(address _t) public returns (uint){
        a = Addition(_t);
        additionResult = a.add(10, 5);
        return additionResult;
    }
}
