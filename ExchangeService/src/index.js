require('dotenv').config();
const logger = require('./utils/logger');
const bitgetService = require('./exchanges/bitget');
const xtService = require('./exchanges/xt');
const gateService = require('./exchanges/gate');

logger.info('Starting Exchange Service...');

// 启动 Bitget 服务
bitgetService.start();
logger.info('Bitget service started.');

// 启动 XT.COM 服务
xtService.start();
logger.info('XT.COM service started.');

gateService.start();
logger.info('Gate.io service started.');

logger.info('All exchange services are running.');