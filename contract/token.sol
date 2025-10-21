// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Capped.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract NexTradeDAO is ERC20, ERC20Capped, ERC20Burnable, ERC20Permit, Ownable {
    uint256 private constant DECIMALS = 18;
    uint256 private constant TOKEN_UNIT = 10**DECIMALS;
    uint256 private constant SECONDS_PER_DAY = 86400;

    uint256 private constant DAYS_PHASE1 = 20 * 365;
    uint256 private constant DAYS_PHASE2 = 30 * 365;
    uint256 private constant TOTAL_DAYS = DAYS_PHASE1 + DAYS_PHASE2;

    uint256 private constant I1 = 76712328767123287671232;
    uint256 private constant I0 = 383561643835616438356165;
    uint256 private constant I0_MINUS_I1 = I0 - I1;

    uint256 private constant TOTAL_SUPPLY = 3_000_000_000 * TOKEN_UNIT;
    uint256 private constant TEAM_ALLOCATION = (TOTAL_SUPPLY * 15) / 100; // 15%
    uint256 private constant PRIVATE_ALLOCATION = (TOTAL_SUPPLY * 10) / 100; // 10%
    uint256 private constant COMMUNITY_ALLOCATION = (TOTAL_SUPPLY * 5) / 100; // 5%
    uint256 private constant TEAM_VESTING_MONTHS = 20 * 12; // 240 months
    uint256 private constant PRIVATE_VESTING_MONTHS = 10 * 12; // 120 months
    uint256 private constant COMMUNITY_VESTING_MONTHS = 20 * 12; // 240 months
    uint256 private constant SECONDS_PER_MONTH = 30 * SECONDS_PER_DAY;

    address[103] public projectAddresses;
    uint256 public immutable startDate;
    uint256 public lastMintDay;
    uint256 public lastVestingMonth;
    uint256 private _randomNonce;
    bool private _initializing;

    constructor(
        uint256 _initialDay,
        address[100] memory _initialProjectAddresses,
        address _teamAddress,
        address _privateAddress,
        address _communityAddress,
        address _initialOwner
    ) 
        ERC20("NexTrade DAO", "NTX") 
        ERC20Capped(3_000_000_000 * TOKEN_UNIT) 
        ERC20Burnable() 
        ERC20Permit("NexTrade DAO") 
        Ownable(_initialOwner) 
    {
        require(_initialDay < TOTAL_DAYS, "Initial day out of bounds");

        _initializing = true;
        startDate = block.timestamp - _initialDay * SECONDS_PER_DAY;
        
        // Set the first 100 project addresses
        for (uint i = 0; i < 100; i++) {
            projectAddresses[i] = _initialProjectAddresses[i];
        }
        
        // Set the special addresses
        projectAddresses[100] = _teamAddress;
        projectAddresses[101] = _privateAddress;
        projectAddresses[102] = _communityAddress;

        if (_initialDay > 0) {
            uint256 initialMintAmount = _calculateMintAmount(0, _initialDay - 1);
            if (initialMintAmount > 0) {
                _distributeAndMint(initialMintAmount);
            }
            lastMintDay = _initialDay - 1;
            
            uint256 initialMonths = (_initialDay * SECONDS_PER_DAY) / SECONDS_PER_MONTH;
            if (initialMonths > 0) {
                _mintVesting(0, initialMonths - 1);
                lastVestingMonth = initialMonths - 1;
            }
        } else {
            uint256 dayZeroAmount = getDailyIssuance(0);
            if (dayZeroAmount > 0) {
                _distributeAndMint(dayZeroAmount);
            }
            lastMintDay = 0;
        }
        
        _initializing = false;
    }

    function _update(address from, address to, uint256 value) internal override(ERC20, ERC20Capped) {
        if (!_initializing) {
            _triggerMint();
            _triggerVesting();
        }
        super._update(from, to, value);
    }

    function setProjectAddresses(address[100] memory _newAddresses, address _teamAddress, address _privateAddress, address _communityAddress) external onlyOwner {
        for (uint i = 0; i < 100; i++) {
            projectAddresses[i] = _newAddresses[i];
        }
        projectAddresses[100] = _teamAddress;
        projectAddresses[101] = _privateAddress;
        projectAddresses[102] = _communityAddress;
    }

    function getDailyIssuance(uint256 _day) public pure returns (uint256) {
        if (_day >= TOTAL_DAYS) {
            return 0;
        }
        if (_day < DAYS_PHASE1) {
            return I0 - (I0_MINUS_I1 * _day) / DAYS_PHASE1;
        } else {
            uint256 dayInPhase2 = _day - DAYS_PHASE1;
            return I1 - (I1 * dayInPhase2) / DAYS_PHASE2;
        }
    }

    function _triggerMint() private {
        uint256 currentDay = _getCurrentDay();
        if (currentDay <= lastMintDay) {
            return; // No new days have passed
        }
        
        uint256 endMintDay = currentDay < TOTAL_DAYS ? currentDay : TOTAL_DAYS - 1;

        if (endMintDay <= lastMintDay) {
            return;
        }

        uint256 totalToMint = _calculateMintAmount(lastMintDay + 1, endMintDay);

        if (totalToMint > 0) {
            _distributeAndMint(totalToMint);
        }

        lastMintDay = endMintDay;
    }

    function _triggerVesting() private {
        uint256 currentMonth = _getCurrentMonth();
        if (currentMonth <= lastVestingMonth) {
            return;
        }
        
        _mintVesting(lastVestingMonth + 1, currentMonth);
        lastVestingMonth = currentMonth;
    }

    function _mintVesting(uint256 _startMonth, uint256 _endMonth) private {
        if (_startMonth > _endMonth) return;
        
        // Team (addr 100): 15%, 240 months
        if (_startMonth < TEAM_VESTING_MONTHS) {
            uint256 endMonth = _endMonth < TEAM_VESTING_MONTHS ? _endMonth : TEAM_VESTING_MONTHS - 1;
            uint256 months = endMonth - _startMonth + 1;
            uint256 amount = (TEAM_ALLOCATION * months) / TEAM_VESTING_MONTHS;
            if (amount > 0) {
                _mint(projectAddresses[100], amount);
            }
        }
        
        // Private (addr 101): 10%, 120 months
        if (_startMonth < PRIVATE_VESTING_MONTHS) {
            uint256 endMonth = _endMonth < PRIVATE_VESTING_MONTHS ? _endMonth : PRIVATE_VESTING_MONTHS - 1;
            uint256 months = endMonth - _startMonth + 1;
            uint256 amount = (PRIVATE_ALLOCATION * months) / PRIVATE_VESTING_MONTHS;
            if (amount > 0) {
                _mint(projectAddresses[101], amount);
            }
        }
        
        // Community (addr 102): 5%, 240 months
        if (_startMonth < COMMUNITY_VESTING_MONTHS) {
            uint256 endMonth = _endMonth < COMMUNITY_VESTING_MONTHS ? _endMonth : COMMUNITY_VESTING_MONTHS - 1;
            uint256 months = endMonth - _startMonth + 1;
            uint256 amount = (COMMUNITY_ALLOCATION * months) / COMMUNITY_VESTING_MONTHS;
            if (amount > 0) {
                _mint(projectAddresses[102], amount);
            }
        }
    }

    function _distributeAndMint(uint256 _totalAmount) private {
        uint256 ownerShare = (_totalAmount * 10) / 100;
        uint256 projectTotalShare = _totalAmount - ownerShare;

        uint256 dust = _totalAmount % 100;
        if (ownerShare + dust > 0) {
            _mint(owner(), ownerShare + dust);
        }

        if (projectTotalShare == 0) {
            return;
        }

        uint256[100] memory weights;
        uint256 totalWeight = 0;
        
        for (uint i = 0; i < 100; i++) {
            uint256 salt = i + _randomNonce;
            uint256 weight = uint256(keccak256(abi.encodePacked(block.timestamp, address(this), salt))) % 1000 + 1; // Weight between 1 and 1000
            weights[i] = weight;
            totalWeight += weight;
        }
        _randomNonce++;

        uint256 mintedForProjects = 0;
        for (uint i = 0; i < 99; i++) {
            uint256 share = (projectTotalShare * weights[i]) / totalWeight;
            if (share > 0) {
                 _mint(projectAddresses[i], share);
                 mintedForProjects += share;
            }
        }
        
        uint256 remainingShare = projectTotalShare - mintedForProjects;
        if (remainingShare > 0) {
            _mint(projectAddresses[99], remainingShare);
        }
    }

    function _calculateMintAmount(uint256 _startDay, uint256 _endDay) private pure returns (uint256) {
        if (_startDay > _endDay) return 0;

        if (_endDay >= TOTAL_DAYS) {
            _endDay = TOTAL_DAYS - 1;
        }

        if (_endDay < DAYS_PHASE1) {
            uint256 firstTerm = getDailyIssuance(_startDay);
            uint256 lastTerm = getDailyIssuance(_endDay);
            uint256 numTerms = _endDay - _startDay + 1;
            return (numTerms * (firstTerm + lastTerm)) / 2;
        }

        if (_startDay >= DAYS_PHASE1) {
            uint256 firstTerm = getDailyIssuance(_startDay);
            uint256 lastTerm = getDailyIssuance(_endDay);
            uint256 numTerms = _endDay - _startDay + 1;
            return (numTerms * (firstTerm + lastTerm)) / 2;
        }

        uint256 p1_end = DAYS_PHASE1 - 1;
        uint256 p1_firstTerm = getDailyIssuance(_startDay);
        uint256 p1_lastTerm = getDailyIssuance(p1_end);
        uint256 p1_numTerms = p1_end - _startDay + 1;
        uint256 p1_sum = (p1_numTerms * (p1_firstTerm + p1_lastTerm)) / 2;

        uint256 p2_start = DAYS_PHASE1;
        uint256 p2_firstTerm = getDailyIssuance(p2_start);
        uint256 p2_lastTerm = getDailyIssuance(_endDay);
        uint256 p2_numTerms = _endDay - p2_start + 1;
        uint256 p2_sum = (p2_numTerms * (p2_firstTerm + p2_lastTerm)) / 2;

        return p1_sum + p2_sum;
    }

    function _getCurrentDay() private view returns (uint256) {
        if (block.timestamp < startDate) return 0;
        return (block.timestamp - startDate) / SECONDS_PER_DAY;
    }

    function _getCurrentMonth() private view returns (uint256) {
        if (block.timestamp < startDate) return 0;
        return (block.timestamp - startDate) / SECONDS_PER_MONTH;
    }
}