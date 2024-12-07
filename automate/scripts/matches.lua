-- Lua script to find matches and publish to a channel
local searcher_keys = redis.call('KEYS', '*:searchers')

local max_elo_diff = 10000 -- TODO: Change this to the actuall value found in the config struct

-- TODO: This function can be optimized by assuming only 2 players. Review this later

local function can_play_together(players)
    for i = 1, #players do
        local player = players[i]

        if tonumber(redis.call('GET', player .. ':matching')) then
            return false
        end

        if tonumber(redis.call('GET', player .. ':ai')) then
            return false
        end

        local player_elo = tonumber(redis.call('GET', player .. ':elo'))
        local player_mode = redis.call('GET', player .. ':mode')
        local player_game = redis.call('GET', player .. ':game')
        local player_region = redis.call('GET', player .. ':region')

        for j = i + 1, #players do
            local other = players[j]

            if tonumber(redis.call('GET', other .. ':matching')) then
                return false
            end

            if tonumber(redis.call('GET', other .. ':ai')) then
                return false
            end


            if redis.call('GET', other .. ':game') ~= player_game then
                return false
            end

            if redis.call('GET', other .. ':mode') ~= player_mode then
                return false
            end

            if redis.call('GET', other .. ':region') ~= player_region then
                return false
            end

            if math.abs(player_elo - tonumber(redis.call('GET', other .. ':elo'))) > max_elo_diff then
                return false
            end
        end
    end
    return true
end


local function handle_match(region, players)
    local uuid = redis.call('INCR', 'uuid_inc')

    redis.call('PUBLISH', uuid .. ':match:region', region)
    for y, other in ipairs(players) do
        redis.call('SET', other .. ':matching', 1)

        redis.call('PUBLISH', uuid .. ':match:players:' .. tostring(y), other)
    end

    redis.call('PUBLISH', uuid .. ':match:done', #players)
end

local function fill_with_ai(players, max_player_count)
    for j = #players + 1, max_player_count do
        local player = players[1]
        local ai = redis.sha1hex(player .. tostring(players)) ..
            "-ai@" .. redis.call('GET', player .. ":game") .. "-" .. redis.call('GET', player .. ":mode")
        table.insert(players, ai)
    end
end

local function check_for_ai_search(player)
    if tonumber(redis.call('GET', player .. ':matching')) then
        return false
    end

    if tonumber(redis.call('GET', player .. ':ai')) == 0 then
        return false
    end


    local players = { player }

    fill_with_ai(players, redis.call('GET', player .. ':max_players'))


    handle_match(redis.call('GET', player .. ':region'), players)
end

-- Match all ai players
for i = 1, #searcher_keys do
    local player = searcher_keys[i]
    check_for_ai_search(player)
end



-- Match all non-ai players
for i = 1, #searcher_keys do
    local player1 = searcher_keys[i]

    local max_player_count = tonumber(redis.call('GET', player1 .. ':max_players'))
    local min_player_count = tonumber(redis.call('GET', player1 .. ':min_players'))

    local players = { player1 }
    for j = i + 1, #searcher_keys do
        local player2 = searcher_keys[j]

        if can_play_together({ player1, player2 }) then
            table.insert(players, player2)
            if #players == max_player_count then
                break
            end
        end
    end

    local region = redis.call('GET', player1 .. ':region')
    if #players >= min_player_count then
        handle_match(region, players)
    else
        if tonumber(redis.call('INCR', player1 .. ':failed_searches')) == 10 then
            redis.call('SET', player1 .. ':fill_with_ai', 0)

            local all_fill_with_ai = true

            for _, player in ipairs(players) do
                if not redis.call('GET', player .. ':fill_with_ai') then
                    all_fill_with_ai = false
                    break
                end
            end

            if all_fill_with_ai then
                for _, player in ipairs(players) do
                    redis.call('SET', player .. ':ai', 1)
                end

                fill_with_ai(players, max_player_count)
                handle_match(region, players)
            end
        end
    end
end
