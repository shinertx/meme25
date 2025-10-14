import os
import requests
import json

api_key = os.getenv("JUPITER_API_KEY")
base_url = os.getenv("JUPITER_API_URL")

if not api_key or not base_url:
    print("--- ERROR ---")
    print("JUPITER_API_KEY or JUPITER_API_URL not found in environment.")
    exit(1)

url = f"{base_url}/quote"
params = {
    "inputMint": "So11111111111111111111111111111111111111112",
    "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    "amount": 1000000,
    "slippageBps": 50
}
headers = {
    "Content-Type": "application/json",
    "x-api-key": api_key
}

print(f"--- Testing Jupiter Pro API Key ---")
print(f"URL: {url}")
print(f"API Key: ...{api_key[-4:]}")

try:
    resp = requests.get(url, params=params, headers=headers, timeout=15)
    print(f"\nStatus Code: {resp.status_code}\n")
    print("--- Response Body ---")
    print(resp.text)
    
    if resp.status_code == 200 and "outAmount" in resp.text:
        print("\n--- VERDICT: ✅ VALID ---")
    else:
        print(f"\n--- VERDICT: ❌ INVALID ---")
        print(f"Jupiter responded with status {resp.status_code}. Check your API key and plan.")

except requests.exceptions.RequestException as e:
    print(f"\n--- REQUEST FAILED ---")
    print(f"An error occurred: {e}")
    print("\n--- VERDICT: ❌ UNKNOWN (Network Error) ---")
