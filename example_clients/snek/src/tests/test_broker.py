import unittest
from unittest.mock import MagicMock
import random

from src.broker import BrokerBuilder, Order, OrderType


def generate_fake_quotes(symbols, date):
    quotes = []
    for symbol in symbols:
        price = random.randint(10, 20)
        quote_dict = {
            "bid": price,
            "bid_volume": random.randint(100, 1000),
            "ask": price + 1,
            "ask_volume": random.randint(100, 1000),
            "date": date,
            "symbol": symbol,
        }
        quotes.append(quote_dict)
    return {"quotes": quotes}


class TestBroker(unittest.TestCase):
    def test_main_loop(self):
        http_client = MagicMock()

        http_client.init.return_value = {"backtest_id": 0}
        http_client.fetch_quotes.side_effect = [
            generate_fake_quotes(["ABC"], 100),
            generate_fake_quotes(["ABC"], 101),
        ]
        http_client.tick.return_value = {
            "has_next": False,
            "executed_trades": [],
            "inserted_orders": [],
        }

        builder = BrokerBuilder()
        builder.init_cash(1000)
        builder.init_dataset_name("Test")
        builder.init_http(http_client)
        brkr = builder.build()

        order = Order(OrderType.MarketBuy, "ABC", 100, None)
        brkr.insert_order(order)

        brkr.tick()
