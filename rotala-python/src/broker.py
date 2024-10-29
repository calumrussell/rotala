from enum import Enum
import json
import logging

from src.http import HttpClient

logger = logging.getLogger(__name__)


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

    def __str__(self):
        return f"{self.order_type} {self.symbol} {self.qty} {self.price}"

    def serialize(self):
        if self.price:
            return f'{{"order_type": "{self.order_type.name}", "symbol": "{self.symbol}", "qty": {self.qty}, "price": {self.price}, "recieved": 0}}'
        else:
            return f'{{"order_type": "{self.order_type.name}", "symbol": "{self.symbol}", "qty": {self.qty}, "price": null, "recieved": 0}}'

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
        self,
        symbol: str,
        value: float,
        quantity: float,
        date: int,
        typ: TradeType,
        order_id: int,
    ):
        self.symbol = symbol
        self.value = value
        self.quantity = quantity
        self.date = date
        self.typ = typ
        self.order_id = order_id

    def __str__(self):
        return (
            f"{self.typ} {self.order_id} - {self.quantity}/{self.value} {self.symbol}"
        )

    @staticmethod
    def from_dict(trade_dict: dict):
        if trade_dict["typ"] == "Buy":
            return Trade(
                trade_dict["symbol"],
                trade_dict["value"],
                trade_dict["quantity"],
                trade_dict["date"],
                TradeType.Buy,
                trade_dict["order_id"],
            )
        else:
            return Trade(
                trade_dict["symbol"],
                trade_dict["value"],
                trade_dict["quantity"],
                trade_dict["date"],
                TradeType.Sell,
                trade_dict["order_id"],
            )

    @staticmethod
    def from_json(json_str: str):
        to_dict = json.loads(json_str)
        return Trade(
            to_dict["symbol"],
            to_dict["value"],
            to_dict["quantity"],
            to_dict["date"],
            to_dict["typ"],
            to_dict["order_id"],
        )


class Broker:
    def __init__(self, builder: BrokerBuilder):
        self.builder = builder
        self.http = builder.http
        self.cash = builder.initial_cash
        self.dataset_name = builder.dataset_name
        self.holdings = {}
        self.pending_orders = []
        self.trade_log = []
        self.order_inserted_on_last_tick = []
        self.unexecuted_orders = {}
        self.portfolio_values = []
        self.backtest_id = None
        self.ts = None

        # Initializes backtest_id, can ignore result
        init_response = self.http.init(self.dataset_name)
        self.backtest_id = init_response["backtest_id"]
        quotes_resp = self.http.fetch_quotes()
        self.latest_quotes = quotes_resp["quotes"]
        self.ts = list(self.latest_quotes.values())[0]["date"]

    def _update_holdings(self, position: str, chg: float):
        if position not in self.holdings:
            self.holdings[position] = 0

        curr_position = self.holdings[position]
        new_position = curr_position + chg
        logger.info(
            f"{self.backtest_id}-{self.ts} POSITION CHG: {position} {curr_position} -> {new_position}"
        )
        self.holdings[position] = new_position

    def _validate_order(self, order) -> bool:
        if (
            order.order_type == OrderType.MarketSell
            or order.order_type == OrderType.LimitSell
        ):
            curr_position = self.holdings[order.symbol]

            if curr_position == 0:
                return False

            if order.qty > curr_position:
                return False
        return True

    def _process_trade(self, trade: Trade):
        logger.info(f"{self.backtest_id}-{self.ts} EXECUTED: {trade}")

        before_trade = self.cash
        after_trade = (
            self.cash - trade.value
            if trade.typ == TradeType.Buy
            else self.cash + trade.value
        )

        logger.info(
            f"{self.backtest_id}-{self.ts} CASH: {before_trade} -> {after_trade}"
        )
        self.cash = after_trade
        if self.cash < 0:
            logger.critical("Run out of cash. Stopping sim.")
            exit(1)

        signed_qty = trade.quantity if trade.typ == TradeType.Buy else -trade.quantity
        self._update_holdings(trade.symbol, signed_qty)

    def insert_order(self, order: Order):
        # Orders are only flushed when we call tick
        self.pending_orders.append(order)

    def get_quotes(self):
        return self.latest_quotes

    def get_position(self, symbol) -> float:
        return self.holdings.get(symbol, 0)

    def get_current_value(self) -> float:
        value = self.cash
        # This will fail if there is a missing quote for the date
        # Fix is to cache values in self.latest_quotes
        for symbol in self.holdings:
            quote = self.latest_quotes[symbol]
            if quote:
                qty = self.holdings[symbol]
                symbol_bid = quote["bid"]
                value += qty * symbol_bid
        return value

    def tick(self):
        logger.info(f"{self.backtest_id}-{self.ts} TICK")

        # Flush pending orders
        while len(self.pending_orders) > 0:
            order = self.pending_orders.pop()
            if self._validate_order(order):
                logger.info(f"{self.backtest_id}-{self.ts} INSERT ORDER: {order}")
                self.http.insert_order(order)
            else:
                logger.info(
                    f"{self.backtest_id}-{self.ts} FAILED INSERT ORDER: {order}"
                )

        # Tick, reconcile our state
        self.order_inserted_on_last_tick = []
        tick_response = self.http.tick()
        for trade_json in tick_response["executed_trades"]:
            trade = Trade.from_dict(trade_json)
            # This should always be the case
            if trade.order_id in self.unexecuted_orders:
                order = self.unexecuted_orders[trade.order_id]
                if trade.quantity > order["qty"]:
                    order["quantity"] -= trade.quantity
                else:
                    del self.unexecuted_orders[trade.order_id]

            self._process_trade(trade)
            self.trade_log.append(trade)

        for order in tick_response["inserted_orders"]:
            self.unexecuted_orders[order["order_id"]] = order
            self.order_inserted_on_last_tick.append(order)

        if not tick_response["has_next"]:
            logger.critical("Sim finished")
            exit(0)
        else:
            self.latest_quotes = self.http.fetch_quotes()["quotes"]
            if self.latest_quotes:
                self.ts = list(self.latest_quotes.values())[0]["date"]

        curr_value = self.get_current_value()
        logger.info(f"{self.backtest_id}-{self.ts} TOTAL VALUE: {curr_value}")
        self.portfolio_values.append(curr_value)
