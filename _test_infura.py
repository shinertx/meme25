import os
import requests
import json

# The shell script running this will have already sourced the .env file
api_key = os.getenv("INFURA_API_KEY")
if not api_key:
    print("--- ERROR ---")
    print("INFURA_API_KEY not found in environment.")
    exit(1)

url = f"https://mainnet.infura.io/v3/{api_key}"
payload = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "eth_blockNumber"
}
headers = {
    "Content-Type": "application/json"
}

print(f"--- Testing URL: {url} ---\n")

try:
    resp = requests.post(url, headers=headers, data=json.dumps(payload), timeout=10)
    print(f"Status Code: {resp.status_code}\n")
    print("--- Response Body ---\n")
    print(resp.text)
    if resp.status_code == 200 and "result" in resp.text:
        print("\n--- VERDICT: ✅ VALID ---")
    else:
        print(f"\n--- VERDICT: ❌ INVALID ---")
        print(f"Infura responded with status {resp.status_code}. Check project settings (JWT, IP Whitelisting, Mainnet enabled).")

except requests.exceptions.RequestException as e:
    print(f"--- REQUEST FAILED ---")
    print(f"An error occurred: {e}")
    print("\n--- VERDICT: ❌ UNKNOWN (Network Error) ---")
