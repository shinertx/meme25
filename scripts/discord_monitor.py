#!/usr/bin/env python3
"""
Discord Live Trading Monitor for MemeSnipe v25
Sends real-time updates to your Discord channel
"""

import requests
import json
import time
import os
from datetime import datetime

# Replace with your Discord webhook URL
DISCORD_WEBHOOK_URL = "https://discord.com/api/webhooks/YOUR_WEBHOOK_HERE"

def send_discord_message(message, title="📊 MemeSnipe v25 Live Update"):
    """Send formatted message to Discord"""
    payload = {
        "embeds": [{
            "title": title,
            "description": message,
            "color": 0x00ff00,  # Green
            "timestamp": datetime.utcnow().isoformat(),
            "footer": {"text": "MemeSnipe v25 Live Monitor"}
        }]
    }
    
    try:
        response = requests.post(DISCORD_WEBHOOK_URL, json=payload)
        return response.status_code == 204
    except Exception as e:
        print(f"Discord send failed: {e}")
        return False

def get_trading_metrics():
    """Fetch current trading metrics"""
    try:
        # Try to get metrics from executor
        response = requests.get("http://localhost:9091/metrics", timeout=5)
        if response.status_code == 200:
            metrics = response.text
            
            # Parse key metrics
            portfolio_value = "200.00"  # Default
            total_trades = "0"
            pnl = "0.00"
            
            # Extract real values if available
            for line in metrics.split('\n'):
                if 'portfolio_value' in line:
                    portfolio_value = line.split()[-1]
                elif 'total_trades' in line:
                    total_trades = line.split()[-1]
                elif 'pnl_total' in line:
                    pnl = line.split()[-1]
            
            return {
                "portfolio_value": portfolio_value,
                "total_trades": total_trades,
                "pnl": pnl,
                "status": "🟢 LIVE"
            }
    except:
        pass
    
    return {
        "portfolio_value": "200.00",
        "total_trades": "0", 
        "pnl": "0.00",
        "status": "🔴 STARTING"
    }

def main():
    """Main monitoring loop"""
    print("🚀 Starting Discord live monitor...")
    
    # Send startup message
    send_discord_message(
        "🚀 **MemeSnipe v25 LIVE MONITORING STARTED**\n\n"
        "📊 Target: $200 → $1,000,000\n"
        "🎯 Mode: Paper Trading with Real Data\n"
        "⚡ Updates every 30 seconds",
        "🟢 System Online"
    )
    
    while True:
        try:
            metrics = get_trading_metrics()
            
            message = (
                f"💰 **Portfolio Value**: ${metrics['portfolio_value']}\n"
                f"🔄 **Total Trades**: {metrics['total_trades']}\n"
                f"📈 **P&L**: ${metrics['pnl']}\n"
                f"🟢 **Status**: {metrics['status']}\n\n"
                f"⏰ **Last Update**: {datetime.now().strftime('%H:%M:%S')}"
            )
            
            send_discord_message(message)
            time.sleep(30)  # Update every 30 seconds
            
        except KeyboardInterrupt:
            send_discord_message(
                "🛑 **MONITORING STOPPED**\n\nLive monitoring has been manually stopped.",
                "🔴 System Offline"
            )
            break
        except Exception as e:
            print(f"Monitor error: {e}")
            time.sleep(5)

if __name__ == "__main__":
    main()
