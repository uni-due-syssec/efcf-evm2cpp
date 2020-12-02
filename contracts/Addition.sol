pragma solidity >=0.7.0 <0.9.0;
contract Addition {
    function add(uint256 num1, uint256 num2) public pure returns (uint256){
        uint256 number = num1 + num2;
        return number;
    }
}
