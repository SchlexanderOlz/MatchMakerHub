import redis
import time
import os 

SEARCHERS_SCRIPT_DIR = os.getenv('SEARCHERS_SCRIPT_DIR')
with open(SEARCHERS_SCRIPT_DIR, 'r') as file:
    searchers_lua_script = file.read()

r = redis.StrictRedis(host=os.getenv('REDIS_HOST'), port=int(os.getenv('REDIS_PORT')), db=0)

searchers_script_sha = r.script_load(searchers_lua_script)

def log_match_results(result):
    if not result or len(result) < 2:
        return

    searcher_count = result[0]
    match_entries = result[1]

    if match_entries:
        game_totals = {}
        for entry in match_entries:
            game = entry[0].decode() if isinstance(entry[0], bytes) else entry[0]
            count = entry[1]
            game_totals[game] = game_totals.get(game, 0) + count
        for game, total in game_totals.items():
            print(f"Match found for {total} users in game {game}", flush=True)


while True:
    start_time = time.time()

    try:
        result = r.evalsha(searchers_script_sha, 0)
    except redis.exceptions.ResponseError as e:
        print(e, flush=True)
        result = r.eval(searchers_lua_script, 0)

    log_match_results(result)

    elapsed_time = time.time() - start_time

    time.sleep(max(0, 1 - elapsed_time))