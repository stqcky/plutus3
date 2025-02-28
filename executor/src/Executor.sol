// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "forge-std/console.sol";
import {IERC20} from "lib/IERC20.sol";

contract Executor {
    address payable private _owner;

    bool private _locked = true;
    bool private _insideFirstTrade = false;

    constructor() {
        _owner = payable(msg.sender);
    }

    function execute1695833(address target, bytes calldata data) external payable {
        require(msg.sender == _owner, "fuck off");

        _locked = false;
        _insideFirstTrade = true;

        (bool success, bytes memory _data) = target.call(data);

        if (!success) {
            if (_data.length == 0) revert("data is zero");

            assembly {
                revert(add(32, _data), mload(_data))
            }
        }

        _locked = true;
    }

    function _processSwap(bytes calldata data) private {
        require(!_locked, "fuck off");

        if (_insideFirstTrade) {
            _insideFirstTrade = false;

            (address recipient, address token, uint256 amount, bytes memory extra) =
                abi.decode(data, (address, address, uint256, bytes));

            (address[] memory targets, bytes[] memory _data) = abi.decode(extra, (address[], bytes[]));

            _multicall(targets, _data);

            IERC20(token).transfer(recipient, amount);
            IERC20(token).transfer(_owner, IERC20(token).balanceOf(address(this)));
        } else {
            (address recipient, address token, uint256 amount) = abi.decode(data, (address, address, uint256));
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

    function uniswapV3SwapCallback(int256, int256, bytes calldata data) external {
        _processSwap(data);
    }

    function uniswapV2Call(address, uint256, uint256, bytes calldata data) external {
        _processSwap(data);
    }

    function pancakeCall(address, uint256, uint256, bytes calldata data) external {
        _processSwap(data);
    }
}
