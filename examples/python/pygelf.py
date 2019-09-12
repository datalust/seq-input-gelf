# Example logging with https://github.com/keeprocking/pygelf
# First, `pip install pygelf`

from pygelf import GelfUdpHandler
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger()
logger.addHandler(GelfUdpHandler(host='127.0.0.1', port=12201))

logger.info('Hello, from Python!')
