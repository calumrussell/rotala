import requests


class Http:
    def __init__(self, base_url):
        self.base_url = base_url
        return

    def tick(self, backtest_id):
        r = requests.get(f"{self.base_url}/backtest/{backtest_id}/tick")
        return r.json()

    def insert_order(self, order, backtest_id):
        r = requests.post(
            f"{self.base_url}/backtest/{backtest_id}/insert_order",
            data={"order": order},
        )
        return r.json()

    def fetch_quotes(self, backtest_id):
        r = requests.get(f"{self.base_url}/backtest/{backtest_id}/fetch_quotes")
        return r.json()

    def init(self, dataset_name):
        r = requests.get(f"{self.base_url}/init/{dataset_name}")
        return r.json()

    def info(self, backtest_id):
        r = requests.get(f"{self.base_url}/backtest/{backtest_id}/info")
        return r.json()

    def now(self, backtest_id):
        r = requests.get(f"{self.base_url}/backtest/{backtest_id}/now")
        return r.json()
