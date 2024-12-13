import redis
import time
import os 

SEARCHERS_SCRIPT_DIR = os.getenv('SEARCHERS_SCRIPT_DIR')
with open(SEARCHERS_SCRIPT_DIR, 'r') as file:
    searchers_lua_script = file.read()

r = redis.StrictRedis(host=os.getenv('REDIS_HOST'), port=int(os.getenv('REDIS_PORT')), db=0)

searchers_script_sha = r.script_load(searchers_lua_script)

while True:
    start_time = time.time()
    
    try:
        r.evalsha(searchers_script_sha, 0)
    except redis.exceptions.ResponseError as e:
        print(e)
        r.eval(searchers_lua_script, 0)

    elapsed_time = time.time() - start_time
    
    time.sleep(max(0, 1 - elapsed_time))