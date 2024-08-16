import redis
import time

with open('/usr/matchmaking-state/scripts/matches.lua', 'r') as file:
    lua_script = file.read()

r = redis.StrictRedis(host='monitoring_db', port=6379, db=0)

script_sha = r.script_load(lua_script)

while True:
    start_time = time.time()
    
    r.evalsha(script_sha, 0)
    elapsed_time = time.time() - start_time
    
    time.sleep(max(0, 1 - elapsed_time))