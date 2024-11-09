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
    MarketSell = "MarketSell"
    MarketBuy = "MarketBuy"
    LimitBuy = "LimitBuy"
    LimitSell = "LimitSell"
    Modify = "Modify"
    Cancel = "Cancel"


class Order:
    def __init__(
        self,
        order_type: OrderType,
        symbol: str,
        qty: float,
        price: float | None,
        order_id_ref: float | None,
    ):
        if order_type == OrderType.MarketSell or order_type == OrderType.MarketBuy:
            if price is not None:
                raise ValueError("Order price must be None for Market order")

        self.order_type = order_type
        self.symbol = symbol
        self.qty = qty
        self.price = price
        self.order_id_ref = order_id_ref

    def __str__(self):
        return f"{self.order_type} {self.symbol} {self.qty} {self.price}"

    def is_transaction(self) -> bool:
        return (
            self.order_type == OrderType.LimitBuy
            or self.order_type == OrderType.LimitSell
            or self.order_type == OrderType.MarketBuy
            or self.order_type == OrderType.MarketSell
        )

    def serialize(self):
        base = f'{{"order_type": "{self.order_type.name}", "symbol": "{self.symbol}", "qty": {self.qty}'
        if self.price:
            base += f', "price": {self.price}'
        if self.order_id_ref is not None:
            base += f', "order_id_ref": {self.order_id_ref}'
        base += "}"
        return base

    @staticmethod
    def from_dict(order):
        order_type = OrderType(order["order_type"])
        return Order(
            order_type,
            order["symbol"],
            order["qty"],
            order["price"],
            order["order_id_ref"],
        )

    @staticmethod
    def from_json(json_str):
        to_dict = json.loads(json_str)
        return Order(
            to_dict["order_type"],
            to_dict["symbol"],
            to_dict["qty"],
            to_dict["price"],
            to_dict["order_id_ref"],
        )


class OrderResultType(Enum):
    Buy = "Buy"
    Sell = "Sell"
    Modify = "Modify"
    Cancel = "Cancel"


class OrderResult:
    def __init__(
        self,
        symbol: str,
        value: float,
        quantity: float,
        date: int,
        typ: OrderResultType,
        order_id: int,
        order_id_ref: int | None,
    ):
        self.symbol = symbol
        self.value = value
        self.quantity = quantity
        self.date = date
        self.typ = typ
        self.order_id = order_id
        self.order_id_ref = order_id_ref

    def __str__(self):
        return (
            f"{self.typ} {self.order_id} - {self.quantity}/{self.value} {self.symbol}"
        )

    @staticmethod
    def from_dict(from_dict: dict):
        trade_type = OrderResultType(from_dict["typ"])
        return OrderResult(
            from_dict["symbol"],
            from_dict["value"],
            from_dict["quantity"],
            from_dict["date"],
            trade_type,
            from_dict["order_id"],
            from_dict["order_id_ref"],
        )

    @staticmethod
    def from_json(json_str: str):
        to_dict = json.loads(json_str)
        return OrderResult(
            to_dict["symbol"],
            to_dict["value"],
            to_dict["quantity"],
            to_dict["date"],
            to_dict["typ"],
            to_dict["order_id"],
            to_dict["order_id_ref"],
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
        self.latest_quotes = init_response["bbo"]
        self.latest_depth = init_response["depth"]
        self.ts = list(self.latest_quotes.values())[0]["date"]

    def _update_holdings(self, position: str, chg: float):
        if position not in self.holdings:
            self.holdings[position] = 0

        curr_position = self.holdings[position]
        new_position = curr_position + chg
        logger.debug(
            f"{self.backtest_id}-{self.ts} POSITION CHG: {position} {curr_position} -> {new_position}"
        )
        self.holdings[position] = new_position

    def _process_order_result(self, result: OrderResult):
        logger.debug(f"{self.backtest_id}-{self.ts} EXECUTED: {result}")

        if result.typ == OrderResultType.Buy or result.typ == OrderResultType.Sell:
            before_trade = self.cash
            after_trade = (
                self.cash - result.value
                if result.typ == OrderResultType.Buy
                else self.cash + result.value
            )

            logger.debug(
                f"{self.backtest_id}-{self.ts} CASH: {before_trade} -> {after_trade}"
            )
            self.cash = after_trade
            if self.cash < 0:
                logger.critical("Run out of cash. Stopping sim.")
                exit(1)

            signed_qty = (
                result.quantity
                if result.typ == OrderResultType.Buy
                else -result.quantity
            )
            self._update_holdings(result.symbol, signed_qty)

            if result.order_id in self.unexecuted_orders:
                order = self.unexecuted_orders[result.order_id]
                if result.quantity > order.qty:
                    order["quantity"] -= result.quantity
                else:
                    del self.unexecuted_orders[result.order_id]
        else:
            if result.typ == OrderResultType.Cancel:
                del self.unexecuted_orders[result.order_id]
                del self.unexecuted_orders[result.order_id_ref]
            else:
                logger.critical("Unsupported order modification type")
                exit(1)

    def insert_order(self, order: Order):
        # Orders are only flushed when we call tick
        self.pending_orders.append(order)

    def get_quotes(self):
        return self.latest_quotes

    def get_depth(self):
        return self.latest_depth

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
        logger.debug(
            f"{self.backtest_id}-{self.ts} INSERTING {len(self.pending_orders)} ORDER"
        )
        self.http.insert_orders(self.pending_orders)
        self.pending_orders = []

        # Tick, reconcile our state
        self.order_inserted_on_last_tick = []
        tick_response = self.http.tick()
        for order_result_json in tick_response["executed_orders"]:
            order_result = OrderResult.from_dict(order_result_json)
            self._process_order_result(order_result)
            self.trade_log.append(order_result)

        for order in tick_response["inserted_orders"]:
            self.unexecuted_orders[order["order_id"]] = Order.from_dict(order)
            self.order_inserted_on_last_tick.append(order)

        if not tick_response["has_next"]:
            logger.critical("Sim finished")
            exit(0)
        else:
            self.latest_quotes = tick_response["bbo"]
            self.latest_depth = tick_response["depth"]
            if self.latest_quotes:
                self.ts = list(self.latest_quotes.values())[0]["date"]

        curr_value = self.get_current_value()
        logger.debug(f"{self.backtest_id}-{self.ts} TOTAL VALUE: {curr_value}")
        self.portfolio_values.append(curr_value)
