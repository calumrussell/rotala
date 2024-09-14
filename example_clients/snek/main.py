from src.broker import BrokerBuilder, Order, OrderType
from src.http import HttpClient


def calc_avg(last):
    return sum([quote["ask"] for quote in last]) / len(last)


if __name__ == "__main__":
    last_five = []

    builder = BrokerBuilder()
    builder.init_dataset_name("Test")
    builder.init_cash(10000)
    builder.init_http(HttpClient("http://127.0.0.1:8080"))
    brkr = builder.build()

    should_continue = True

    while should_continue:
        quotes = brkr.latest_quotes
        print(quotes)
        for quote in quotes:
            symbol_quote = quotes[quote]
            if symbol_quote["symbol"] == "ABC":
                if len(last_five) < 5:
                    last_five.append(quote)
                else:
                    avg = calc_avg(last_five)
                    curr = quote["ask"]
                    if curr * 0.95 < avg and brkr.get_position("ABC") == 0:
                        order = Order(OrderType.MarketBuy, "ABC", 1.0, None)
                        brkr.insert_order(order)
                    elif avg > curr and brkr.get_position("ABC") > 0:
                        order = Order(
                            OrderType.MarketSell, "ABC", brkr.get_position("ABC"), None
                        )
                        brkr.insert_order(order)
        brkr.tick()
