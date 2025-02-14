// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {CallbackValidation} from "lib/CallbackValidation.sol";
import {IERC20} from "lib/IERC20.sol";

contract Executor {
    address private constant UNISWAP_V3_FACTORY = 0x1F98431c8aD98523631AE4a59f267346ea31F984;

    address payable private _owner;

    constructor() {
        _owner = payable(msg.sender);
    }

    function execute(address[] calldata _targets, bytes[] calldata _data) external {
        require(msg.sender == _owner, "fuck off");

        uint256 length = _targets.length;
        for (uint256 i = 0; i < length;) {
            (bool success,) = _targets[i].call(_data[i]);

            require(success, "not success");

            unchecked {
                ++i;
            }
        }
    }

    function uniswapV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes calldata data) external {
        require(amount0Delta > 0 || amount1Delta > 0, "zero amount swap");
        (address tokenIn, address tokenOut, uint24 fee) = abi.decode(data, (address, address, uint24));

        CallbackValidation.verifyCallback(UNISWAP_V3_FACTORY, tokenIn, tokenOut, fee);

        (bool isExactInput, uint256 amountToPay) =
            amount0Delta > 0 ? (tokenIn < tokenOut, uint256(amount0Delta)) : (tokenOut < tokenIn, uint256(amount1Delta));

        if (isExactInput) {
            IERC20(tokenIn).transfer(msg.sender, amountToPay);
        } else {
            IERC20(tokenOut).transfer(msg.sender, amountToPay);
        }
    }
}
