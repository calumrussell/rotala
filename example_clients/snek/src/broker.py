from enum import Enum
import json

from src.http import HttpClient


class BrokerBuilder:
    def __init__(self):
        self.initial_cash = None
        self.http = None
        self.dataset_name = None

    def init_cash(self, value: int):
        self.initial_cash = value

    def init_http(self, http: HttpClient):
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
        self, order_type: OrderType, symbol: str, qty: float, price: float | None
    ):
        if order_type == OrderType.MarketSell or order_type == OrderType.MarketBuy:
            if price is not None:
                raise ValueError("Order price must be None for Market order")
        else:
            if price is None:
                raise ValueError("Order price must be not None for Limit order")

        if qty <= 0:
            raise ValueError("Order qty must be greater than zero")

        self.order_type = order_type
        self.symbol = symbol
        self.qty = qty
        self.price = price

    def serialize(self):
        return json.dumps(self)

    @staticmethod
    def from_json(json_str):
        to_dict = json.loads(json_str)
        Order(
            to_dict["order_type"],
            to_dict["symbol"],
            to_dict["symbol"],
            to_dict["qty"],
            to_dict["price"],
        )


class TradeType(Enum):
    Buy = 0
    Sell = 1


class Trade:
    def __init__(
        self, symbol: str, value: float, quantity: float, date: int, typ: TradeType
    ):
        self.symbol = symbol
        self.value = value
        self.quantity = quantity
        self.date = date
        self.typ = typ

    @staticmethod
    def from_json(json_str: str):
        to_dict = json.loads(json_str)
        return Trade(
            to_dict["symbol"],
            to_dict["value"],
            to_dict["quantity"],
            to_dict["date"],
            to_dict["typ"],
        )


class Broker:
    def __init__(self, builder: BrokerBuilder):
        self.builder = builder
        self.http = builder.http
        self.cash = builder.initial_cash
        self.dataset_name = builder.dataset_name
        self.holdings = {}
        self.pending_orders = []
        self.finished = False
        self.trade_log = []
        self.order_log = []

        # Initializes backtest_id, can ignore result
        self.http.init(self.dataset_name)
        quotes_resp = self.http.fetch_quotes()
        self.latest_quotes = quotes_resp["quotes"]

    def _update_holdings(self, position: str, chg: float):
        if position not in self.holdings:
            self.holdings[position] = 0
        self.holdings[position] += chg

    def _validate_order(self, order) -> bool:
        if (
            order.order_type == OrderType.MarketSell
            or order.order_type == OrderType.LimitSell
        ):
            curr_position = self.holdings[order.symbol]
            if curr_position == 0 or order.qty > curr_position:
                return False
        return True

    def _process_trade(self, trade: Trade):
        self.cash = self.cash - trade.value
        signed_qty = trade.quantity if trade.typ == TradeType.Buy else -trade.quantity

        self._update_holdings(trade.symbol, signed_qty)

    def insert_order(self, order: Order):
        if self.finished:
            return

        # Orders are only flushed when we call tick
        self.pending_orders.append(order)

    def get_quotes(self):
        return self.latest_quotes

    def get_position(self, symbol):
        return self.holdings[symbol]

    def tick(self):
        if self.finished:
            print("Sim finished, cannot tick again so exiting.")
            exit(0)

        while len(self.pending_orders) > 0:
            ##TODO: fails silently if validation fails, should log error
            order = self.pending_orders.pop()
            if self._validate_order(order):
                self.http.insert_order(order)

        tick_response = self.http.tick()
        for trade_json in tick_response["executed_trades"]:
            trade = Trade.from_json(trade_json)
            self._process_trade(trade)
            self.trade_log.append(trade)

        for order in tick_response["inserted_orders"]:
            self.order_log.append(order)

        if not tick_response["has_next"]:
            self.finished = True
        else:
            self.latest_quotes = self.http.fetch_quotes()["quotes"]
