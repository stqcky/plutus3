// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {CallbackValidation} from "lib/CallbackValidation.sol";
import {IERC20} from "lib/IERC20.sol";
import {UniswapV2Library} from "lib/UniswapV2Library.sol";

contract Executor {
    address private constant UNISWAP_V3_FACTORY = 0x1F98431c8aD98523631AE4a59f267346ea31F984;
    address private constant UNISWAP_V2_FACTORY = 0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9;
    address private constant PANCAKE_v2_FACTORY = 0x02a84c1b3BBD7401a5f7fa98a384EBC70bB5749E;

    address payable private _owner;

    bool private executing = false;

    constructor() {
        _owner = payable(msg.sender);
    }

    function execute(address _target, bytes calldata _data) external {
        require(msg.sender == _owner, "fuck off");

        _target.call(_data);
    }

    function multicall(address[] memory _targets, bytes[] memory _data) private {
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

        address tokenIn;
        address tokenOut;
        uint24 fee;

        if (!executing) {
            executing = true;

            (address _tokenIn, address _tokenOut, uint24 _fee, address[] memory _targets, bytes[] memory _data) =
                abi.decode(data, (address, address, uint24, address[], bytes[]));

            multicall(_targets, _data);

            tokenIn = _tokenIn;
            tokenOut = _tokenOut;
            fee = _fee;
        } else {
            (tokenIn, tokenOut, fee) = abi.decode(data, (address, address, uint24));
        }

        // (address tokenIn, address tokenOut, uint24 fee) = abi.decode(data, (address, address, uint24));

        CallbackValidation.verifyCallback(UNISWAP_V3_FACTORY, tokenIn, tokenOut, fee);

        (bool isExactInput, uint256 amountToPay) =
            amount0Delta > 0 ? (tokenIn < tokenOut, uint256(amount0Delta)) : (tokenOut < tokenIn, uint256(amount1Delta));

        if (isExactInput) {
            IERC20(tokenIn).transfer(msg.sender, amountToPay);
        } else {
            IERC20(tokenOut).transfer(msg.sender, amountToPay);
        }
    }

    function uniswapV2Call(address sender, uint256 amount0, uint256 amount1, bytes calldata data) external {
        revert("uni v2");
        // require(amount0 > 0 || amount1 > 0, "zero amount swap");
        //
        // (address tokenIn, address tokenOut) = abi.decode(data, (address, address));
        // require(msg.sender == UniswapV2Library.pairFor(UNISWAP_V2_FACTORY, tokenIn, tokenOut), "!UNIV2");
        //
        // (bool isExactInput, uint256 amountToPay) =
        //     amount0 > 0 ? (tokenIn < tokenOut, amount0) : (tokenOut < tokenIn, amount1);
        //
        // if (isExactInput) {
        //     IERC20(tokenIn).transfer(msg.sender, amountToPay);
        // } else {
        //     IERC20(tokenOut).transfer(msg.sender, amountToPay);
        // }
    }

    function pancakeCall(address sender, uint256 amount0, uint256 amount1, bytes calldata data) external {
        revert("pancake");
        // require(amount0 > 0 || amount1 > 0, "zero amount swap");
        //
        // (address tokenIn, address tokenOut) = abi.decode(data, (address, address));
        // require(msg.sender == UniswapV2Library.pairFor(UNISWAP_V2_FACTORY, tokenIn, tokenOut), "!UNIV2");
        //
        // (bool isExactInput, uint256 amountToPay) =
        //     amount0 > 0 ? (tokenIn < tokenOut, amount0) : (tokenOut < tokenIn, amount1);
        //
        // if (isExactInput) {
        //     IERC20(tokenIn).transfer(msg.sender, amountToPay);
        // } else {
        //     IERC20(tokenOut).transfer(msg.sender, amountToPay);
        // }
    }
}
