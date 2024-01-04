Implements a very simple trading strategy in Python that buys 100 units whenever current price
goes under 95% of 5-period moving average. Sells all units whenever it goes over and position
isn't zero.

``
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python src/main.py
``