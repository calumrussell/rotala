import random

from snake import PyQuote, staticweight_example

import snake

data = {}
tickers = ["ABC", "BCD"]
for i in range(1, 100_000):
    tmp = []
    for ticker in tickers:
        price = random.randint(90, 110)
        quote = PyQuote(bid=price, ask=price, date=i, symbol=ticker)
        tmp.append(quote)
    data[i] = tmp

tickers_dict = {ticker: i for i, ticker in enumerate(tickers)}

res = snake.staticweight_example(data, {}, tickers_dict)
print(res)
