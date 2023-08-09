import random

import snake

data = {}
tickers = ["ABC", "BCD"]
dates = []
for i in range(100, 1000):
    data[i] = [random.randint(90, 110) for ticker in tickers]

res = snake.staticweight_example(data)
print(res)
