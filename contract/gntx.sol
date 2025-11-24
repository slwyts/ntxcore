// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC1155/ERC1155.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract GNTXNFT is ERC1155, Ownable {
    // 代币元数据URI
    string public tokenURI;
    // NTX ERC20代币合约地址
    address public ntxTokenAddress;
    // 用户绑定信息
    mapping(address => string) public userBindings;
    // 用户铸造GNTX NFT的最大数量
    uint256 public maxMintPerUser;
    // GNTX NFT的最大供应量
    uint256 public maxSupply;
    // 已铸造的GNTX NFT数量
    uint256 public totalMinted;
    // 已销毁的GNTX NFT数量
    uint256 public totalBurned;
    // 质押NTX的数量，用于换取1个GNTX NFT（常规单位ether）
    uint256 public ntxPerNFT;

    event NFTMinted(address indexed user, uint256 indexed tokenId, uint256 amount);
    event NFTBurned(address indexed user, uint256 indexed tokenId, uint256 amount);
    event UserBindingUpdated(address indexed user, string email);
    event MaxMintPerUserUpdated(uint256 newMaxMintPerUser);
    event MaxSupplyUpdated(uint256 newMaxSupply);
    event NtxPerNFTUpdated(uint256 newNtxPerNFT);
    event TokenUriUpdated(string newTokenUri);
    // 构造函数
    constructor(
        string memory _tokenURI, 
        address _ntxTokenAddress, 
        address _initialOwner,
        uint256 _ntxPerNFT,
        uint256 _maxMintPerUser,
        uint256 _maxSupply
    ) ERC1155(_tokenURI) Ownable(_initialOwner) {
        tokenURI = _tokenURI;
        ntxTokenAddress = _ntxTokenAddress;
        maxMintPerUser = _maxMintPerUser; // 动态设置每个用户最多铸造的GNTX NFT数量
        maxSupply = _maxSupply; // 动态设置GNTX NFT的最大供应量
        ntxPerNFT = _ntxPerNFT * (10 ** 18); // 设置质押NTX的数量
    }

    // 用户绑定邮箱
    // function bindUser(address _user, string memory _email) external onlyOwner {
    //     userBindings[_user] = _email;
    //     emit UserBindingUpdated(_user, _email);
    // }

    // 修改用户铸造GNTX NFT的最大数量
    function updateMaxMintPerUser(uint256 _newMaxMintPerUser) external onlyOwner {
        maxMintPerUser = _newMaxMintPerUser;
        emit MaxMintPerUserUpdated(_newMaxMintPerUser);
    }

    // 修改GNTX NFT的最大供应量
    function updateMaxSupply(uint256 _newMaxSupply) external onlyOwner {
        maxSupply = _newMaxSupply;
        emit MaxSupplyUpdated(_newMaxSupply);
    }

    // 修改质押NTX的数量（常规单位ether）
    function updateNtxPerNFT(uint256 _newNtxPerNFT) external onlyOwner {
        ntxPerNFT = _newNtxPerNFT * (10 ** 18);
        emit NtxPerNFTUpdated(_newNtxPerNFT);
    }

    // 修改token uri
    function updateTokenUri(string memory _newTokenUri) external onlyOwner {
        tokenURI = _newTokenUri;
        emit TokenUriUpdated(_newTokenUri);
    }

    // 用户质押NTX代币铸造GNTX NFT
    function mintNFT(uint256 _amount) external {
        //require(bytes(userBindings[msg.sender]).length > 0, "User is not bound");
        require(balanceOf(msg.sender, 1) + _amount <= maxMintPerUser, "Exceeds max mint per user");
        require(totalMinted - totalBurned + _amount <= maxSupply, "Exceeds max supply");

        // 计算所需的NTX数量（转换为最小单位）
        uint256 requiredNtx = _amount * ntxPerNFT;

        // 确保用户已经批准了足够的代币额度
        uint256 allowance = IERC20(ntxTokenAddress).allowance(msg.sender, address(this));
        require(allowance >= requiredNtx, "Insufficient allowance");

        IERC20(ntxTokenAddress).transferFrom(msg.sender, address(this), requiredNtx); // 质押NTX代币
        _mint(msg.sender, 1, _amount, "");
        totalMinted += _amount;

        emit NFTMinted(msg.sender, 1, _amount);
    }

    // 用户解除质押并销毁GNTX NFT
    function burnNFT(uint256 _amount) external {
        require(balanceOf(msg.sender, 1) >= _amount, "Insufficient balance");

        _burn(msg.sender, 1, _amount);
        totalBurned += _amount;

        uint256 returnNtx = _amount * ntxPerNFT;
        IERC20(ntxTokenAddress).transfer(msg.sender, returnNtx); // 返还NTX代币

        emit NFTBurned(msg.sender, 1, _amount);
    }

    function uri(uint256) public view override returns (string memory) {
        return tokenURI;
    }

    // 禁止NFT转账
    function _beforeTokenTransfer(
        address operator,
        address from,
        address to,
        uint256[] memory ids,
        uint256[] memory amounts,
        bytes memory data
    ) internal {
        require(from == address(0) || to == address(0), "Token transfer is not allowed");
    }
}