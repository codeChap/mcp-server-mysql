import argparse
import subprocess
import json
import sys
import random
import time

def main():
    parser = argparse.ArgumentParser(description="Fuzz test the MCP MySQL Server with random queries.")
    parser.add_argument("--host", default="localhost", help="MySQL host")
    parser.add_argument("--port", type=int, default=3306, help="MySQL port")
    parser.add_argument("--username", required=True, help="MySQL username")
    parser.add_argument("--password", default="", help="MySQL password")
    parser.add_argument("--database", required=True, help="MySQL database")
    parser.add_argument("--count", type=int, default=5, help="Number of random queries to generate")
    
    args = parser.parse_args()

    # Construct the cargo command
    cmd = [
        "cargo", "run", "--quiet", "--bin", "mcp-server-mysql", "--",
        "--host", args.host,
        "--port", str(args.port),
        "--username", args.username,
        "--database", args.database,
        "--password", args.password
    ]

    print(f"Starting server: {' '.join(cmd)}")
    
    # Start the server process
    process = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=sys.stderr, # Pipe stderr to console so we see logs
        text=True,
        bufsize=1
    )

    def send_request(req):
        json_str = json.dumps(req)
        # print(f"> Sending: {json_str}")
        process.stdin.write(json_str + "\n")
        process.stdin.flush()
        
        response_line = process.stdout.readline()
        # print(f"< Received: {response_line.strip()}")
        if not response_line:
            raise Exception("Server closed connection unexpected")
        return json.loads(response_line)

    try:
        # 1. Initialize
        print("\n[1] Initializing...")
        init_req = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {"name": "fuzz-tester", "version": "1.0.0"}
            }
        }
        resp = send_request(init_req)
        if "error" in resp:
            print(f"Initialization failed: {resp['error']}")
            return
        print("Initialized successfully.")

        # 2. Initialized Notification
        process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "initialized"}) + "\n")
        process.stdin.flush()

        # 3. Get Schema (to know valid tables/columns)
        print("\n[2] Fetching Schema...")
        schema_req = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "mysql",
                "arguments": {"table_name": "all-tables"}
            }
        }
        resp = send_request(schema_req)
        
        if "error" in resp:
            print(f"Failed to fetch schema: {resp['error']}")
            return

        # Parse schemas
        # The result content is a text string describing the schemas? 
        # Wait, the tool definition returns "schemas": Vec<Value> in the result block, 
        # but wrapped in the standard MCP "content" text block?
        # Let's check rpc.rs/db.rs implementation.
        # It returns: result: { content: [...], schemas: [...] }
        # The 'schemas' key is extra data alongside the standard content.
        
        schemas = resp.get("result", {}).get("schemas", [])
        if not schemas:
            print("No schemas returned (or empty database). Cannot fuzz queries.")
            # Fallback to simple queries if no tables
            tables = []
        else:
            print(f"Found {len(schemas)} tables.")
            tables = schemas

        # 4. Generate Random Queries
        print(f"\n[3] Running {args.count} random queries...")
        
        for i in range(args.count):
            query = ""
            if not tables or random.random() < 0.2:
                # 20% chance of a generic query (or if no tables)
                query = random.choice([
                    "SELECT 1",
                    "SELECT VERSION()",
                    "SELECT NOW()",
                    "SHOW TABLES"
                ])
            else:
                # Pick a random table
                table = random.choice(tables)
                table_name = table["table_name"]
                columns = [c["name"] for c in table["columns"]]
                
                # Random query type
                q_type = random.choice(["SELECT_ALL", "SELECT_COLS", "COUNT"])
                
                if q_type == "SELECT_ALL":
                    query = f"SELECT * FROM `{table_name}` LIMIT {random.randint(1, 10)}"
                elif q_type == "SELECT_COLS":
                    # Pick 1-3 random columns
                    num_cols = random.randint(1, min(3, len(columns)))
                    selected_cols = random.sample(columns, num_cols)
                    cols_str = ", ".join([f"`{c}`" for c in selected_cols])
                    query = f"SELECT {cols_str} FROM `{table_name}` LIMIT {random.randint(1, 10)}"
                elif q_type == "COUNT":
                    query = f"SELECT COUNT(*) as count FROM `{table_name}`"

            print(f"\nQuery {i+1}: {query}")
            
            query_req = {
                "jsonrpc": "2.0",
                "id": 100 + i,
                "method": "tools/call",
                "params": {
                    "name": "query",
                    "arguments": {"query": query}
                }
            }
            
            start_time = time.time()
            resp = send_request(query_req)
            duration = time.time() - start_time
            
            if "error" in resp:
                print(f"Error: {resp['error']['message']}")
            else:
                # Extract results from content text
                content = resp["result"]["content"][0]["text"]
                # Just print the first line of content (summary)
                print(f"Success ({duration:.3f}s): {content.splitlines()[0]}")

    except KeyboardInterrupt:
        print("\nStopping...")
    except Exception as e:
        print(f"\nError: {e}")
    finally:
        process.terminate()

if __name__ == "__main__":
    main()
