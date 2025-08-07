// gntx-sync-service/index.js

const { ethers } = require('ethers');
const axios = require('axios');
require('dotenv').config(); // 用于加载 .env 文件中的环境变量

// 从环境变量获取配置
const BSC_PROVIDER_URL = process.env.BSC_PROVIDER_URL || 'https://data-seed-prebsc-1-s1.binance.org:8545/'; // BSC 测试网 RPC
const GNTX_NFT_CONTRACT_ADDRESS = process.env.GNTX_NFT_CONTRACT_ADDRESS; // 您的 GNTX NFT 合约地址
const BACKEND_BASE_URL = process.env.BACKEND_BASE_URL || 'http://localhost:3000'; // 后端 API 基础 URL
const GNTX_BALANCE_UPDATE_URL = `${BACKEND_BASE_URL}/api/system/users/gntx_balance`; // 后端更新 GNTX 余额的 API
const ALL_USERS_GNTX_INFO_URL = `${BACKEND_BASE_URL}/api/system/users/gntx_info`; // 后端获取所有用户 GNTX 信息的 API
const ADMIN_API_KEY = process.env.ADMIN_API_KEY; // 用于调用后端管理员 API 的密钥

// 检查必要的环境变量
if (!GNTX_NFT_CONTRACT_ADDRESS || !ADMIN_API_KEY) {
    console.error('错误：请在 .env 文件中设置 GNTX_NFT_CONTRACT_ADDRESS 和 ADMIN_API_KEY。');
    process.exit(1);
}

// GNTX NFT 合约 ABI (包含我们关心的事件和 balanceOf 函数)
const GNTX_NFT_ABI = [
    // NFTMinted(address indexed user, uint256 indexed tokenId, uint256 amount)
    "event NFTMinted(address indexed user, uint256 indexed tokenId, uint256 amount)",
    // NFTBurned(address indexed user, uint256 indexed tokenId, uint256 amount)
    "event NFTBurned(address indexed user, uint256 indexed tokenId, uint256 amount)",
    // userBindings(address) view returns (string)
    "function userBindings(address) view returns (string)",
    // balanceOf(address account, uint256 id) view returns (uint256) - 用于查询 ERC1155 余额
    "function balanceOf(address account, uint256 id) view returns (uint256)",
];

// 初始化 Ethers.js Provider 和 Contract
const provider = new ethers.JsonRpcProvider(BSC_PROVIDER_URL);
const gntxNftContract = new ethers.Contract(GNTX_NFT_CONTRACT_ADDRESS, GNTX_NFT_ABI, provider);

console.log(`BSC Provider URL: ${BSC_PROVIDER_URL}`);
console.log(`GNTX NFT Contract Address: ${GNTX_NFT_CONTRACT_ADDRESS}`);
console.log(`Backend Base URL: ${BACKEND_BASE_URL}`);
console.log(`GNTX Balance Update URL: ${GNTX_BALANCE_UPDATE_URL}`);
console.log(`All Users GNTX Info URL: ${ALL_USERS_GNTX_INFO_URL}`);

/**
 * 获取用户绑定的邮箱地址
 * @param {string} bscAddress 用户的 BSC 地址
 * @returns {Promise<string|null>} 邮箱地址或 null
 */
async function getUserEmailFromBinding(bscAddress) {
    try {
        const email = await gntxNftContract.userBindings(bscAddress);
        return email || null; // 如果未绑定，返回空字符串，将其视为 null
    } catch (error) {
        console.error(`获取地址 ${bscAddress} 的绑定邮箱失败:`, error.message);
        return null;
    }
}

/**
 * 更新后端数据库中用户的 GNTX 余额
 * @param {string} email 用户的邮箱
 * @param {number} gntxBalance 更新后的 GNTX 余额
 */
async function updateBackendGntxBalance(email, gntxBalance) {
    try {
        const response = await axios.put(
            GNTX_BALANCE_UPDATE_URL,
            {
                email: email,
                gntx_balance: gntxBalance,
            },
            {
                headers: {
                    'Content-Type': 'application/json',
                    'X-API-KEY': ADMIN_API_KEY, // 使用管理员 API 密钥进行认证
                },
            }
        );
        console.log(`成功更新用户 ${email} 的 GNTX 余额至 ${gntxBalance}：`, response.data.message);
    } catch (error) {
        if (error.response) {
            console.error(`更新用户 ${email} 的 GNTX 余额失败（状态码 ${error.response.status}）：`, error.response.data);
        } else {
            console.error(`更新用户 ${email} 的 GNTX 余额失败：`, error.message);
        }
    }
}

/**
 * 处理 NFTMinted 事件
 * @param {string} user 铸造者地址
 * @param {ethers.BigNumber} tokenId 代币ID
 * @param {ethers.BigNumber} amount 铸造数量
 * @param {object} event 事件对象
 */
async function handleNFTMinted(user, tokenId, amount, event) {
    // tokenId 1 是 GNTX NFT
    if (tokenId.toString() !== '1') {
        console.log(`  跳过非 GNTX NFT (Token ID: ${tokenId.toString()}) 的 Mint 事件。`);
        return;
    }

    const gntxAmount = parseFloat(ethers.formatUnits(amount, 0)); // GNTX NFT 是非小数位
    console.log(`\n检测到 NFTMinted 事件：`);
    console.log(`  铸造者地址: ${user}`);
    console.log(`  Token ID: ${tokenId.toString()}`);
    console.log(`  铸造数量: ${gntxAmount}`);
    console.log(`  交易哈希: ${event.log.transactionHash}`);

    const userEmail = await getUserEmailFromBinding(user);
    if (userEmail) {
        console.log(`  找到绑定的邮箱: ${userEmail}`);
        try {
            // 查询用户在链上的实际 GNTX NFT 余额 (tokenId 1)
            const userGntxBalanceOnChain = await gntxNftContract.balanceOf(user, 1);
            const formattedBalance = parseFloat(ethers.formatUnits(userGntxBalanceOnChain, 0));
            console.log(`  用户 ${user} 的链上 GNTX NFT 余额: ${formattedBalance}`);
            await updateBackendGntxBalance(userEmail, formattedBalance);
        } catch (error) {
            console.error(`查询用户 ${user} 的链上 GNTX 余额失败:`, error.message);
        }
    } else {
        console.warn(`  铸造者地址 ${user} 未绑定邮箱，跳过后端同步。`);
    }
}

/**
 * 处理 NFTBurned 事件
 * @param {string} user 销毁者地址
 * @param {ethers.BigNumber} tokenId 代币ID
 * @param {ethers.BigNumber} amount 销毁数量
 * @param {object} event 事件对象
 */
async function handleNFTBurned(user, tokenId, amount, event) {
    // tokenId 1 是 GNTX NFT
    if (tokenId.toString() !== '1') {
        console.log(`  跳过非 GNTX NFT (Token ID: ${tokenId.toString()}) 的 Burn 事件。`);
        return;
    }

    const gntxAmount = parseFloat(ethers.formatUnits(amount, 0));
    console.log(`\n检测到 NFTBurned 事件：`);
    console.log(`  销毁者地址: ${user}`);
    console.log(`  Token ID: ${tokenId.toString()}`);
    console.log(`  销毁数量: ${gntxAmount}`);
    console.log(`  交易哈希: ${event.log.transactionHash}`);

    const userEmail = await getUserEmailFromBinding(user);
    if (userEmail) {
        console.log(`  找到绑定的邮箱: ${userEmail}`);
        try {
            // 查询用户在链上的实际 GNTX NFT 余额 (tokenId 1)
            const userGntxBalanceOnChain = await gntxNftContract.balanceOf(user, 1);
            const formattedBalance = parseFloat(ethers.formatUnits(userGntxBalanceOnChain, 0));
            console.log(`  用户 ${user} 的链上 GNTX NFT 余额: ${formattedBalance}`);
            await updateBackendGntxBalance(userEmail, formattedBalance);
        } catch (error) {
            console.error(`查询用户 ${user} 的链上 GNTX 余额失败:`, error.message);
        }
    } else {
        console.warn(`  销毁者地址 ${user} 未绑定邮箱，跳过后端同步。`);
    }
}

/**
 * 在服务启动时，执行链上和后端数据差异的初步同步。
 * 它会查询所有用户的 GNTX 余额，并与链上数据进行比对和更新。
 */
async function performInitialSync() {
    console.log('\n--- 正在执行初始链上数据同步 ---');
    try {
        const response = await axios.get(
            ALL_USERS_GNTX_INFO_URL,
            {
                headers: {
                    'X-API-KEY': ADMIN_API_KEY, // 使用管理员 API 密钥进行认证
                },
            }
        );

        const usersGntxInfo = response.data;
        console.log(`从后端获取到 ${usersGntxInfo.length} 条用户 GNTX 信息。`);

        for (const userInfo of usersGntxInfo) {
            const { email, bscAddress, gntxBalance: backendGntxBalance } = userInfo;

            if (bscAddress) {
                try {
                    const userGntxBalanceOnChain = await gntxNftContract.balanceOf(bscAddress, 1); // 查询 tokenId 为 1 的 GNTX NFT 余额
                    const formattedOnChainBalance = parseFloat(ethers.formatUnits(userGntxBalanceOnChain, 0));

                    if (formattedOnChainBalance !== backendGntxBalance) {
                        console.log(`  发现数据差异：用户 ${email} (BSC: ${bscAddress})`);
                        console.log(`    后端余额: ${backendGntxBalance}, 链上余额: ${formattedOnChainBalance}`);
                        await updateBackendGntxBalance(email, formattedOnChainBalance);
                    } else {
                        // console.log(`  用户 ${email} 数据一致: ${formattedOnChainBalance}`); // 可选：打印一致的日志
                    }
                } catch (chainError) {
                    console.error(`  查询用户 ${email} (BSC: ${bscAddress}) 链上余额失败:`, chainError.message);
                }
            } else {
                // 如果后端记录中没有 BSC 地址，但 GNTX 余额不为 0，这可能是一个问题，但这里暂时跳过
                if (backendGntxBalance > 0) {
                     console.warn(`  用户 ${email} 在后端有 GNTX 余额 (${backendGntxBalance}) 但未绑定 BSC 地址。跳过链上同步检查。`);
                } else {
                    // console.log(`  用户 ${email} 未绑定 BSC 地址，跳过链上同步检查。`); // 可选：打印跳过日志
                }
            }
        }
        console.log('--- 初始链上数据同步完成 ---');
    } catch (error) {
        console.error('执行初始链上数据同步失败：', error.message);
        if (error.response) {
            console.error('后端响应数据：', error.response.data);
        }
    }
}


/**
 * 启动事件监听器
 */
async function startListening() {
    // 1. 在启动时执行初始数据同步
    await performInitialSync();

    // 2. 监听链上事件
    try {
        // 监听 NFTMinted 事件
        gntxNftContract.on("NFTMinted", handleNFTMinted);
        console.log("正在监听 GNTX NFT 合约的 NFTMinted 事件...");

        // 监听 NFTBurned 事件
        gntxNftContract.on("NFTBurned", handleNFTBurned);
        console.log("正在监听 GNTX NFT 合约的 NFTBurned 事件...");

        // 简单的心跳，确保服务正在运行
        setInterval(() => {
            console.log(`[${new Date().toISOString()}] 服务正在运行，等待事件...`);
        }, 60 * 60 * 1000); // 每小时打印一次心跳

    } catch (error) {
        console.error("启动事件监听器失败:", error);
        // 尝试重新连接或进行错误恢复
        setTimeout(startListening, 5000); // 5秒后重试
    }
}

// 启动服务
startListening();

