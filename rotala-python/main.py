import logging

from src.broker import BrokerBuilder, Order, OrderType
from src.http import HttpClient


def calc_avg(last):
    return sum([quote["ask"] for quote in last]) / len(last)


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)

    last_five = []

    builder = BrokerBuilder()
    builder.init_dataset_name("Test")
    builder.init_cash(10000)
    builder.init_http(HttpClient("http://127.0.0.1:3000"))
    brkr = builder.build()

    should_continue = True

    last_five = []
    last_five_depth = []

    while True:
        last_five.append(brkr.latest_quotes["SOL"])
        last_five = last_five[:5]

        last_five_depth.append(brkr.latest_depth["SOL"])

        avg = calc_avg(last_five)
        curr = brkr.latest_quotes["SOL"]["ask"]

        if curr * 0.95 < avg and brkr.get_position("SOL") == 0:
            order = Order(OrderType.MarketBuy, "SOL", 0.1, None)
            brkr.insert_order(order)
        elif avg > curr and brkr.get_position("SOL") > 0:
            order = Order(OrderType.MarketSell, "SOL", brkr.get_position("SOL"), None)
            brkr.insert_order(order)
        brkr.tick()
