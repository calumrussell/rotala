from src.broker import BrokerBuilder, Order, OrderType
from src.http import HttpClient


def calc_avg(last):
    return sum([quote["ask"] for quote in last]) / len(last)


if __name__ == "__main__":
    last_five = []

    builder = BrokerBuilder()
    builder.init_dataset_name("Test")
    builder.init_cash(10000)
    builder.init_http(HttpClient("http://127.0.0.1:3000"))
    brkr = builder.build()

    should_continue = True

    while should_continue:
        quotes = brkr.latest_quotes
        for quote in quotes:
            symbol_quote = quotes[quote]
            if symbol_quote["symbol"] == "SOL":
                if len(last_five) < 5:
                    last_five.append(symbol_quote)
                else:
                    avg = calc_avg(last_five)
                    curr = symbol_quote["ask"]
                    if curr * 0.95 < avg and brkr.get_position("SOL") == 0:
                        order = Order(OrderType.MarketBuy, "SOL", 1.0, None)
                        brkr.insert_order(order)
                    elif avg > curr and brkr.get_position("SOL") > 0:
                        order = Order(
                            OrderType.MarketSell, "SOL", brkr.get_position("ABC"), None
                        )
                        brkr.insert_order(order)
        brkr.tick()
