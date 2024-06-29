from broker import Broker

def calc_avg(quotes):
    prices = [quote["ask"] for quote in quotes]
    return sum(prices) / 5

if __name__ == "__main__":

    last_five = []

    broker = Broker("http://127.0.0.1:8080")
    should_continue = True

    while should_continue:
        print(broker.get_position("ABC"))
        print(broker.cash_balance)
        quotes = broker.fetch_quotes()
        for quote in quotes:
            if quote["symbol"] == "ABC":
                if len(last_five) < 5:
                    last_five.append(quote)
                else:
                    avg = calc_avg(last_five)
                    curr = quote["ask"]
                    if curr * 0.95 < avg and broker.get_position("ABC") == 0:
                        order = {
                            "order_id": None,
                            "order_type": "MarketBuy",
                            "symbol": "ABC",
                            "shares": 100.0,
                            "price": None
                        }
                        broker.insert_order(order)
                    elif avg > curr and broker.get_position("ABC") > 0:
                        order = {
                            "order_id": None,
                            "order_type": "MarketSell",
                            "symbol": "ABC",
                            "shares": broker.get_position("ABC"),
                            "price": None
                        }
                        broker.insert_order(order)
                if not broker.tick():
                    should_continue = False
