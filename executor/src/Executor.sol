// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "forge-std/console.sol";
import {CallbackValidation} from "lib/CallbackValidation.sol";
import {IERC20} from "lib/IERC20.sol";
import {UniswapV2Library} from "lib/UniswapV2Library.sol";

contract Executor {
    address private constant UNISWAP_V3_FACTORY = 0x1F98431c8aD98523631AE4a59f267346ea31F984;
    address private constant UNISWAP_V2_FACTORY = 0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9;
    address private constant PANCAKE_V2_FACTORY = 0x02a84c1b3BBD7401a5f7fa98a384EBC70bB5749E;

    address payable private _owner;

    bool private _locked = true;
    bool private _insideFirstTrade = false;

    constructor() {
        _owner = payable(msg.sender);
    }

    function execute(address target, bytes calldata data) external returns (bytes memory data) {
        require(msg.sender == _owner, "fuck off");

        _locked = false;
        _insideFirstTrade = true;

        (bool success, bytes memory data) = target.call(data);

        if (!success) {
            if (data.length == 0) revert("data is zero");

            assembly {
                revert(add(32, data), mload(data))
            }
        }

        _locked = true;
    }

    function _processSwap(bytes calldata data) private {
        require(!_locked, "fuck off");

        if (_insideFirstTrade) {
            _insideFirstTrade = false;

            (address recipient, address token, uint256 amount, bytes extra) =
                abi.decode(data, (address, address, uint256, bytes));

            (address[] memory targets, bytes[] memory data) = abi.decode(extra, (address[], bytes[]));

            _multicall(targets, data);

            IERC20(token).transfer(recipient, amount);
            IERC20(token).transfer(_owner, IERC20(token).balanceOf(address(this)));
        } else {
            (recipient, token, amount) = abi.decode(data, (address, address, uint256));
            IERC20(token).transfer(recipient, amount);
        }
    }

    function _multicall(address[] memory targets, bytes[] memory data) private {
        uint256 length = targets.length;

        for (uint256 i = 0; i < length;) {
            (bool success,) = targets[i].call(data[i]);

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

        console.log("i am here");

        if (!executing) {
            console.log("executing");
            executing = true;

            console.log("decode 1");
            (address _tokenIn, address _tokenOut, uint24 _fee, bytes memory extra) =
                abi.decode(data, (address, address, uint24, bytes));

            console.log("decode 2");
            (address[] memory targets, bytes[] memory data) = abi.decode(extra, (address[], bytes[]));

            console.log("multicall");

            console.log("token in: %s %s", IERC20(_tokenIn).symbol(), IERC20(_tokenIn).balanceOf(address(this)));
            console.log("token out: %s %s", IERC20(_tokenOut).symbol(), IERC20(_tokenOut).balanceOf(address(this)));

            multicall(targets, data);

            tokenIn = _tokenIn;
            tokenOut = _tokenOut;
            fee = _fee;
        } else {
            console.log("not executing");
            (tokenIn, tokenOut, fee) = abi.decode(data, (address, address, uint24));
        }

        console.log("verifying callback");

        // (address tokenIn, address tokenOut, uint24 fee) = abi.decode(data, (address, address, uint24));

        CallbackValidation.verifyCallback(UNISWAP_V3_FACTORY, tokenIn, tokenOut, fee);

        console.log("sending tokens");

        (bool isExactInput, uint256 amountToPay) =
            amount0Delta > 0 ? (tokenIn < tokenOut, uint256(amount0Delta)) : (tokenOut < tokenIn, uint256(amount1Delta));

        console.log("token in: %s %s", IERC20(tokenIn).symbol(), IERC20(tokenIn).balanceOf(address(this)));
        console.log("token out: %s %s", IERC20(tokenOut).symbol(), IERC20(tokenOut).balanceOf(address(this)));

        if (isExactInput) {
            IERC20(tokenIn).transfer(msg.sender, amountToPay);
        } else {
            IERC20(tokenOut).transfer(msg.sender, amountToPay);
        }

        console.log("token in: %s %s", IERC20(tokenIn).symbol(), IERC20(tokenIn).balanceOf(address(this)));
        console.log("token out: %s %s", IERC20(tokenOut).symbol(), IERC20(tokenOut).balanceOf(address(this)));
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
