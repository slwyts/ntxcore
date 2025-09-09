const crypto = require('crypto');
const axios = require('axios');
const qs = require('qs');
const logger = require('../../utils/logger'); 

class GateIO {
    /**
     * @param {object} config - API配置
     * @param {string} config.apiKey - 您的 API Key
     * @param {string} config.secretKey - 您的 Secret Key
     * @param {string} [config.baseUrl] - API 的基础 URL
     */
    constructor(config) {
        if (!config.apiKey || !config.secretKey) {
            throw new Error('apiKey 和 secretKey 是必需的');
        }
        this.apiKey = config.apiKey;
        this.secretKey = config.secretKey;
        this.baseUrl = config.baseUrl || 'https://api.gateio.ws';
        this.prefix = '/api/v4';
    }

    /**
     * 生成 APIv4 签名
     * @private
     * @param {string} method - 请求方法 (GET, POST)
     * @param {string} url - 请求路径 (e.g., /spot/tickers)
     * @param {string} queryString - URL 查询参数字符串
     * @param {string} payloadString - 请求体字符串
     * @param {string} timestamp - 时间戳
     * @returns {object} 包含签名信息的请求头
     */
    _generateSignature(method, url, queryString, payloadString, timestamp) {
        const m = crypto.createHash('sha512');
        // --- FIX IS HERE ---
        // Changed from .encode('utf-8') to Buffer.from()
        m.update(Buffer.from(payloadString || ""));
        const hashedPayload = m.digest('hex');

        const s = `${method}\n${url}\n${queryString || ""}\n${hashedPayload}\n${timestamp}`;

        const sign = crypto.createHmac('sha512', this.secretKey).update(s).digest('hex');

        return {
            'KEY': this.apiKey,
            'Timestamp': timestamp,
            'SIGN': sign
        };
    }

    /**
     * 发送 API 请求
     * @param {string} method - 请求方法 (GET, POST)
     * @param {string} path - 请求路径 (e.g., /spot/tickers)
     * @param {object} [params] - URL 查询参数
     * @param {object} [data] - 请求体
     * @returns {Promise<object>} API 响应
     */
    async request(method, path, params = null, data = null) {
        method = method.toUpperCase();
        const timestamp = Math.floor(Date.now() / 1000).toString();
        
        let queryString = '';
        if (params) {
            queryString = qs.stringify(params);
        }

        let payloadString = '';
        if (data) {
            payloadString = JSON.stringify(data);
        }

        const fullPath = this.prefix + path;
        const signHeaders = this._generateSignature(method, fullPath, queryString, payloadString, timestamp);

        const requestConfig = {
            method: method,
            url: `${this.baseUrl}${fullPath}`,
            headers: {
                ...signHeaders,
                'Accept': 'application/json',
                'Content-Type': 'application/json'
            }
        };

        if (params) {
            requestConfig.params = params;
        }

        if (data) {
            requestConfig.data = data;
        }
        // --- 新增日志 ---
        // 打印将要发起的请求的完整 URL，以便调试
        const fullRequestUrl = `${requestConfig.url}${queryString ? '?' + queryString : ''}`;
        logger.info(`[Gate.io] Making request to: ${fullRequestUrl}`);
        // ------------------
        try {
            const response = await axios(requestConfig);
            return response.data;
        } catch (error) {
            if (error.response) {
                console.error(`Error making API request to ${path}:`, error.response ? error.response.data : error.message);
                throw new Error(`API Error: ${error.response.status} ${JSON.stringify(error.response.data)}`);
            } else if (error.request) {
                throw new Error(`API Error: No response received. ${error.message}`);
            } else {
                throw new Error(`API Error: Request setup failed. ${error.message}`);
            }
        }
    }
    
    /**
     * 发送 GET 请求
     * @param {string} path 
     * @param {object} [params] 
     * @returns {Promise<object>}
     */
    async get(path, params = null) {
        return this.request('GET', path, params);
    }

    /**
     * 发送 POST 请求
     * @param {string} path 
     * @param {object} [data] 
     * @returns {Promise<object>}
     */
    async post(path, data = null) {
        return this.request('POST', path, null, data);
    }
}

module.exports = GateIO;