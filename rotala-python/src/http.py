from urllib3.util import Retry
from requests import Session
from requests.adapters import HTTPAdapter


class HttpClient:
    def __init__(self, base_url):
        self.base_url = base_url
        self.backtest_id = None

        s = Session()
        retries = Retry(
            total=3,
            backoff_factor=0.1,
            status_forcelist=[502, 503, 504],
            allowed_methods={"POST"},
        )
        s.mount("https://", HTTPAdapter(max_retries=retries))
        self.s = s
        return

    def init(self, start_date, end_date, frequency):
        val = f'{{"start_date": {start_date}, "end_date": {end_date}, "frequency": {frequency}}}'
        r = self.s.post(
            f"{self.base_url}/init",
            data=val,
            headers={"Content-type": "application/json"},
        )
        json_response = r.json()
        self.backtest_id = int(json_response["backtest_id"])
        return json_response

    def tick(self):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        r = self.s.get(f"{self.base_url}/backtest/{self.backtest_id}/tick")
        return r.json()

    def insert_orders(self, orders):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        serialized_orders_str = ",".join([o.serialize() for o in orders])
        val = f'{{"orders": [{serialized_orders_str}]}}'
        r = self.s.post(
            f"{self.base_url}/backtest/{self.backtest_id}/insert_orders",
            data=val,
            headers={"Content-type": "application/json"},
        )
        return r.json()

    def info(self):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        r = self.s.get(f"{self.base_url}/backtest/{self.backtest_id}/info")
        return r.json()

    def dataset_info(self):
        r = self.s.get(f"{self.base_url}/dataset/info")
        return r.json()
