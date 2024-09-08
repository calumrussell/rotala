from src.http import HttpClient, TestHttpClient
from enum import Enum


class BrokerBuilder:
    def __init__(self):
        self.initial_cash = None
        self.http = None
        self.dataset_name = None

    def init_cash(self, value: int):
        self.initial_cash = value

    def init_http(self, http: HttpClient | TestHttpClient):
        self.http = http

    def init_dataset_name(self, name: str):
        self.dataset_name = name

    def build(self):
        if not self.initial_cash:
            raise ValueError("BrokerBuilder needs cash")

        if not self.http:
            raise ValueError("BrokerBuilder needs http")

        if not self.dataset_name:
            raise ValueError("BrokerBuilder needs dataset name")

        return Broker(self)


class OrderType(Enum):
    MarketSell = 0
    MarketBuy = 1
    LimitBuy = 2
    LimitSell = 3


class Order:
    def __init__(
        self, order_type: OrderType, symbol: str, qty: int, price: float | None
    ):
        if order_type == OrderType.MarketSell or order_type == OrderType.MarketBuy:
            if price is not None:
                raise ValueError("Price must be None for Market order")
        else:
            if price is None:
                raise ValueError("Price must be not None for Limit order")

        self.order_type = order_type
        self.symbol = symbol
        self.qty = qty
        self.price = price


class Broker:
    def __init__(self, builder: BrokerBuilder):
        self.builder = builder
        self.http = builder.http
        self.cash = builder.initial_cash
        self.dataset_name = builder.dataset_name
        self.holdings = {}
        self.pending_orders = []

        # Initializes backtest_id, can ignore result
        self.http.init(self.dataset_name)
        quotes_resp = self.http.fetch_quotes()
        self.latest_quotes = quotes_resp.quotes

    def _update_holdings(self, position: str, chg: float):
        if position not in self.holdings:
            self.holdings[position] = 0
        self.holdings[position] += chg

    def insert_order(self, order: Order):
        # Orders are only flushed when we call tick
        self.pending_orders.append(order)

    def get_quotes(self):
        return self.latest_quotes
