// bitget easyapi.js
const crypto = require('crypto');
const axios = require('axios');
const url = require('url');

class EasyAPI {
    constructor(config) {
        this.apiKey = config.apiKey;
        this.secretKey = config.secretKey;
        this.passphrase = config.passphrase;
        this.baseUrl = config.baseUrl || 'https://api.bitget.com'; // Default Bitget base URL
    }

    /**
     * Generates the HMAC SHA256 signature for the Bitget API.
     * @param {string} timestamp - The timestamp of the request.
     * @param {string} method - The HTTP method (GET/POST).
     * @param {string} requestPath - The API request path.
     * @param {string} queryString - The query string (e.g., "?limit=10&symbol=BTCUSDT").
     * @param {string} body - The request body string.
     * @returns {string} The base64 encoded HMAC SHA256 signature.
     */
    _generateSignature(timestamp, method, requestPath, queryString, body) {
        let preHash = timestamp + method.toUpperCase() + requestPath;

        if (queryString) {
            preHash += "?" + queryString;
        }

        if (body) {
            preHash += body;
        }

        const hmac = crypto.createHmac('sha256', this.secretKey);
        hmac.update(preHash);
        return hmac.digest('base64');
    }

    /**
     * Makes an HTTP request to the Bitget API.
     * @param {string} method - The HTTP method (GET/POST).
     * @param {string} endpoint - The API endpoint (e.g., "/api/mix/v2/market/depth").
     * @param {object} [data] - The data for POST requests (will be stringified to JSON).
     * @param {object} [params] - Query parameters for GET requests.
     * @param {string} [locale='en-US'] - Language setting.
     * @returns {Promise<object>} The API response data.
     */
    async request(method, endpoint, data = null, params = null, locale = 'en-US') {
        const timestamp = Date.now().toString();
        const requestPath = endpoint;

        let queryString = '';
        if (params) {
            queryString = new URLSearchParams(params).toString();
        }

        let bodyString = '';
        if (data && method.toUpperCase() === 'POST') {
            bodyString = JSON.stringify(data);
        }

        const signature = this._generateSignature(timestamp, method, requestPath, queryString, bodyString);

        const headers = {
            'ACCESS-KEY': this.apiKey,
            'ACCESS-SIGN': signature,
            'ACCESS-TIMESTAMP': timestamp,
            'ACCESS-PASSPHRASE': this.passphrase,
            'Content-Type': 'application/json',
            'locale': locale
        };

        const requestConfig = {
            method: method,
            url: `${this.baseUrl}${endpoint}`,
            headers: headers,
        };

        if (method.toUpperCase() === 'GET' && params) {
            requestConfig.params = params;
        } else if (method.toUpperCase() === 'POST' && data) {
            requestConfig.data = data;
        }

        try {
            const response = await axios(requestConfig);
            return response.data;
        } catch (error) {
            console.error(`Error making API request to ${endpoint}:`, error.response ? error.response.data : error.message);
            throw error;
        }
    }

    /**
     * Makes a GET request.
     * @param {string} endpoint - The API endpoint.
     * @param {object} [params] - Query parameters.
     * @param {string} [locale] - Language setting.
     * @returns {Promise<object>} The API response data.
     */
    async get(endpoint, params = null, locale = 'en-US') {
        return this.request('GET', endpoint, null, params, locale);
    }

    /**
     * Makes a POST request.
     * @param {string} endpoint - The API endpoint.
     * @param {object} [data] - The request body data.
     * @param {string} [locale] - Language setting.
     * @returns {Promise<object>} The API response data.
     */
    async post(endpoint, data = null, locale = 'en-US') {
        return this.request('POST', endpoint, data, null, locale);
    }
}

module.exports = EasyAPI;
