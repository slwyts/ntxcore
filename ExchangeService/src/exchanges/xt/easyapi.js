/**
 * @name XT.COM API Easy Client 
 * @description easy xt api by @slwyts
 * @version 1.0.1
 * @dependency axios, crypto, qs
 */

const axios = require('axios');
const crypto = require('crypto');
const qs = require('qs');

/**
 * XT API 客户端类
 */
class XT {
    /**
     * @param {object} config - API配置
     * @param {string} config.apiKey - 您的 API Key
     * @param {string} config.secretKey - 您的 Secret Key
     * @param {string} [config.spotUrl] - 现货 API 的基础 URL
     * @param {string} [config.futureUrl] - 合约 API 的基础 URL
     * @param {string} [config.apiUrl] - 通用/UAPI 的基础 URL
     * @param {number} [config.recvWindow] - 接收窗口时间
     * @param {number} [config.timeout] - 请求超时时间
     */
    constructor(config) {
        if (!config.apiKey || !config.secretKey) {
            throw new Error('apiKey 和 secretKey 是必需的');
        }

        this.apiKey = config.apiKey;
        this.secretKey = config.secretKey;
        this.spotUrl = config.spotUrl || 'https://sapi.xt.com';
        this.futureUrl = config.futureUrl || 'https://fapi.xt.com';
        this.apiUrl = config.apiUrl || 'https://api.xt.com'; // 注意：去掉了末尾的'/'以保持一致性
        this.recvWindow = config.recvWindow || 5000;
        this.timeout = config.timeout || 10000;
    }

    /**
     * 生成现货/通用 API 签名
     * @private
     * @param {string} method - 请求方法 (GET, POST, DELETE)
     * @param {string} path - 请求路径
     * @param {object} headers - 请求头
     * @param {object} [params] - URL 查询参数
     * @param {object} [body] - 请求体
     * @returns {string} 签名字符串
     */
    _generateSpotSignature(method, path, headers, params, body) {
        let signStr = qs.stringify(headers);
        signStr += `#${method.toUpperCase()}#${path}`;

        if (params && Object.keys(params).length) {
            // 参数需要按 key 排序
            signStr += `#${decodeURIComponent(qs.stringify(params, { sort: (a, b) => a.localeCompare(b) }))}`;
        }

        if (body && Object.keys(body).length) {
            signStr += `#${JSON.stringify(body)}`;
        }

        return crypto.createHmac('sha256', this.secretKey).update(signStr).digest('hex');
    }

    /**
     * 生成合约 API 签名
     * @private
     * @param {string} path - 请求路径
     * @param {string} timestamp - 时间戳
     * @param {object} [body] - 请求体
     * @param {object} [batchBody] - 批量请求体
     * @returns {string} 签名字符串
     */
    _generateFutureSignature(path, timestamp, body, batchBody) {
        let signStr = `validate-appkey=${this.apiKey}&validate-timestamp=${timestamp}`;
        signStr += `#${path}`;

        if (body && Object.keys(body).length) {
            signStr += `#${JSON.stringify(body)}`;
        }

        if (batchBody) {
             const batchParams = "list=" + JSON.stringify(batchBody);
             signStr += `#${batchParams}`;
        }
        
        return crypto.createHmac('sha256', this.secretKey).update(signStr).digest('hex');
    }


    /**
     * 发送通用 API 请求
     * @param {object} options - 请求选项
     * @param {string} options.apiType - API 类型 ('spot', 'future', 'uapi', 'api')
     * @param {string} options.method - 请求方法 (GET, POST, DELETE)
     * @param {string} options.path - 请求路径 (例如 /v4/order)
     * @param {object} [options.params] - URL 查询参数
     * @param {object} [options.body] - 请求体 (用于 POST/DELETE)
     * @param {object} [options.batchBody] - 合约批量下单请求体
     * @returns {Promise<object>} API 响应
     */
    async request({ apiType, method, path, params, body, batchBody }) {
        method = method.toUpperCase();
        const timestamp = Date.now();
        
        // --- 修复点 1: 根据 apiType 正确选择 baseUrl ---
        let baseUrl;
        switch (apiType) {
            case 'future':
                baseUrl = this.futureUrl;
                break;
            case 'spot':
                baseUrl = this.spotUrl;
                break;
            case 'uapi':
            case 'api':
                baseUrl = this.apiUrl;
                break;
            default:
                // 默认使用现货地址，或者可以抛出错误
                console.warn(`未知的 apiType: '${apiType}', 将默认使用 spot 地址。`);
                baseUrl = this.spotUrl;
        }

        const requestConfig = {
            method,
            url: `${baseUrl}${path}`,
            timeout: this.timeout,
            headers: {},
        };

        // --- 修复点 2: 重构签名和请求构建逻辑 ---
        if (apiType === 'future') {
            const signature = this._generateFutureSignature(path, timestamp, body, batchBody);
            requestConfig.headers = {
                'Content-Type': batchBody ? 'application/x-www-form-urlencoded' : 'application/json',
                'validate-algorithms': 'HmacSHA256',
                'validate-appkey': this.apiKey,
                'validate-recvwindow': this.recvWindow,
                'validate-timestamp': timestamp,
                'validate-signature': signature
            };
            if(body) requestConfig.data = body;
            if(batchBody) {
                const batchParams = "list=" + encodeURIComponent(JSON.stringify(batchBody));
                requestConfig.url += `?${batchParams}`;
            }

        } else { // 'spot', 'uapi', 'api' 和其他类型都走这个逻辑
            const commonHeaders = {
                'validate-algorithms': 'HmacSHA256',
                'validate-appkey': this.apiKey,
                'validate-recvwindow': this.recvWindow,
                'validate-timestamp': timestamp,
            };
            
            const signature = this._generateSpotSignature(method, path, commonHeaders, params, body);
            requestConfig.headers = { ...commonHeaders, 'validate-signature': signature };
            
            if (params) requestConfig.params = params;
            if (body) requestConfig.data = body;
        }

        try {
            const response = await axios(requestConfig);
            return response.data;
        } catch (error) {
            if (error.response) {
                throw new Error(`API Error: ${error.response.status} ${JSON.stringify(error.response.data)}`);
            } else if (error.request) {
                throw new Error(`API Error: No response received. ${error.message}`);
            } else {
                throw new Error(`API Error: Request setup failed. ${error.message}`);
            }
        }
    }
}

module.exports = XT;
