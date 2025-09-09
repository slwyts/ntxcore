const axios = require('axios');
const logger = require('../utils/logger');

const BACKEND_API_URL = process.env.BACKEND_API_URL;
const ADMIN_API_KEY = process.env.ADMIN_API_KEY;
const IS_TEST_MODE = process.env.MODE === 'test';

if (IS_TEST_MODE) {
    logger.warn('Service is running in TEST MODE. No data will be POSTed to the backend.');
}

/**
 * 推送单条交易数据到后端或在测试模式下仅记录日志。
 * @param {object} tradeData - 包含交易信息的对象
 * @param {string} tradeData.exchangeUid - 用户在交易所的UID
 * @param {number} tradeData.exchangeId - 交易所ID
 * @param {number} tradeData.tradeVolumeUsdt - 交易量
 * @param {number} tradeData.feeUsdt - 手续费
 * @param {string} tradeData.tradeDate - 交易日期 'YYYY-MM-DD'
 * @param {string} exchangeName - 交易所名称 (用于日志)
 */
async function sendTradeData(tradeData, exchangeName) {
    // 根据您提供的后端结构体构建新的 payload
    const payload = {
        exchange_uid: tradeData.exchangeUid.toString(), // 确保UID是字符串
        exchange_id: tradeData.exchangeId,
        trade_volume_usdt: parseFloat(tradeData.tradeVolumeUsdt),
        fee_usdt: parseFloat(tradeData.feeUsdt),
        trade_date: tradeData.tradeDate
    };

    if (IS_TEST_MODE) {
        logger.info(`[TEST MODE] Would push data for ${exchangeName}: ${JSON.stringify(payload)}`);
        return; // 在测试模式下，仅打印日志并直接返回
    }

    try {
        logger.info(`[NORMAL MODE] Submitting trade data for ${exchangeName}: ${JSON.stringify(payload)}`);
        const headers = ADMIN_API_KEY ? { 'X-API-KEY': ADMIN_API_KEY } : {};
        const response = await axios.post(`${BACKEND_API_URL}/admin/add_daily_trade_data`, payload, { headers });

        if (response.status === 200) {
            logger.info(`[NORMAL MODE] Submission successful for ${exchangeName}: ${response.data.message}`);
        } else {
            logger.error(`[NORMAL MODE] Submission failed for ${exchangeName}: Status ${response.status}`);
        }
    } catch (error) {
        logger.error(`[NORMAL MODE] Error calling sendTradeData for ${exchangeName}: ${error.message}`);
        // 打印更详细的错误信息
        if (error.response) {
            logger.error(`[NORMAL MODE] API Error Details: ${JSON.stringify(error.response.data)}`);
        }
    }
}

// fetchUserMapping 函数已完全移除
module.exports = {
    sendTradeData,
};