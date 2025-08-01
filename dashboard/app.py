from flask import Flask, render_template, jsonify
import redis, json, os

app = Flask(__name__)
redis_client = redis.from_url(os.getenv("REDIS_URL", "redis://localhost:6379"), decode_responses=True)

@app.route("/")
def index():
    nav = float(redis_client.get("portfolio_nav") or 0)
    pnl = float(redis_client.get("portfolio_pnl") or 0)
    return render_template("index.html", nav=nav, pnl=pnl)

@app.route("/api/nav")
def api_nav():
    data = {
        "nav": redis_client.get("portfolio_nav") or 0,
        "pnl": redis_client.get("portfolio_pnl") or 0
    }
    return jsonify(data)

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
