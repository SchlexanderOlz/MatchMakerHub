-- Lua script to find matches and publish to a channel
local searcher_keys = redis.call('KEYS', '*:searchers')

local max_elo_diff = 10000 -- TODO: Change this to the actuall value found in the config struct

-- TODO: This function can be optimized by assuming only 2 players. Review this later
local function can_play_together(players)
    for i = 1, #players do
        local player = players[i]

        if redis.call('GET', player + ':matching') then
            return false
        end

        if redis.call('GET', player + ':mode:computer_lobby') then
            return false
        end

        local player_elo = tonumber(redis.call('GET', player + ':elo'))
        local player_mode = redis.call('GET', player + ':mode:name')
        local player_game = redis.call('GET', player + ':game')
        for j = i + 1, #players do
            local other = players[j]

            if redis.call('GET', other + ':matching') then
                return false
            end

            if redis.call('GET', other + ':mode:computer_lobby') then
                return false
            end

            if redis.call('GET', other + ':game') ~= player_game then
                return false
            end

            if redis.call('GET', other + ':mode:name') ~= player_mode then
                return false
            end

            if math.abs(player_elo - tonumber(redis.call('GET', other + ':elo'))) > max_elo_diff then
                return false
            end
        end
    end
    return true
end

local function find_server(players)
    local servers = redis.call('KEYS', players[1] .. ':servers:*')

    -- TODO: This is missing a check for actual server weights. Here the first server is just taken
    for _, server in ipairs(servers) do
        local address = redis.call('GET', server)
        for i = 2, #players do
            local player = players[i]
            local player_servers = redis.call('KEYS', player + ':servers:*')

            for server_key in ipairs(player_servers) do
                if redis.call('GET', server_key) == server then
                    return address
                end
            end
        end
    end
    return nil
end


for i = 1, #searcher_keys do
    local player1 = searcher_keys[i]

    local player_count = tonumber(redis.call('GET', player1 + ':mode:player_count'))

    local players = { player1 }
    for j = i + 1, #searcher_keys do
        local player2 = searcher_keys[j]
        
        if can_play_together({ player1, player2 }) then
            table.insert(players, player2)
            if #players == player_count then
                break
            end
        end
    end

    if #players == player_count then
        local server = find_server(players)
        if server ~= nil then
            local uuid = redis.call('INCR', 'uuid_inc')

            redis.call('PUBLISH', uuid + ':match:server', server)
            for y, other in ipairs(players) do
                redis.call('SET', other + ':matching', true)

                redis.call('PUBLISH', uuid + ':match:players:' + tostring(y), other)
            end

            redis.call('PUBLISH', uuid + ':done', #players)
        end
    end
end