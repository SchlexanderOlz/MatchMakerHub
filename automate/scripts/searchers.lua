-- Lua script to find matches and publish to a channel
local searcher_keys = redis.call('KEYS', '*:searchers')
local ai_players = redis.call('KEYS', '*:ai_players')

local max_elo_diff = 10000 -- TODO: Change this to the actuall value found in the config struct

-- TODO: This function can be optimized by assuming only 2 players. Review this later

local function can_play_together(players)
    for i = 1, #players do
        local player = players[i]

        if tonumber(redis.call('GET', player .. ':matching')) == 1 then
            return false
        end

        if redis.call('GET', player .. ':ai') then
            return false
        end

        local player_elo = tonumber(redis.call('GET', player .. ':elo'))
        local player_mode = redis.call('GET', player .. ':mode')
        local player_game = redis.call('GET', player .. ':game')
        local player_region = redis.call('GET', player .. ':region')

        for j = i + 1, #players do
            local other = players[j]

            if tonumber(redis.call('GET', other .. ':matching')) == 1 then
                return false
            end

            if redis.call('GET', other .. ':ai') then
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


local function publish_new_match(region, player_ids, mode, game, ai)
    local uuid = redis.call('INCR', 'uuid_inc')

    redis.call('PUBLISH', uuid .. ':match:region', region)
    for y, other in ipairs(player_ids) do
        redis.call('SET', other .. ':matching', 1)

        redis.call('PUBLISH', uuid .. ':match:players:' .. tostring(y), other)
    end

    redis.call('PUBLISH', uuid .. ':match:mode', mode)
    redis.call('PUBLISH', uuid .. ':match:game', game)
    redis.call('PUBLISH', uuid .. ':match:ai', ai)

    redis.call('PUBLISH', uuid .. ':match:done', #player_ids)
end


local function handle_match(region, players)
    publish_new_match(region, players, redis.call('GET', players[1] .. ':mode'), redis.call('GET', players[1] .. ':game'), redis.call('GET', players[1] .. ':ai') ~= nil and 1 or 0)
end


local function fill_with_ai(players, max_player_count, preferred_ai)
    if preferred_ai == "." then
        return
    end

    preferred_ai = preferred_ai or '*'

    local game = redis.call('GET', players[1] .. ':game')
    local mode = redis.call('GET', players[1] .. ':mode')

    local eligable_ai = {}


    if preferred_ai ~= '*' then
        for j = 1, #ai_players do
            local ai_player = ai_players[j]

            if redis.call('GET', ai_player .. ":display_name") == preferred_ai then
                table.insert(eligable_ai, ai_player)
                break
            end
        end
    else
        for j = 1, #ai_players do
            local ai_player = ai_players[j]
            if redis.call('GET', ai_player .. ":game") == game and redis.call('GET', ai_player .. ":mode") == mode then
                table.insert(eligable_ai, ai_player)
            end
        end
    end


    if #eligable_ai == 0 then
        return
    end


    for i = #players + 1, max_player_count do
        math.randomseed(redis.call('TIME')[1])

        local ai_player = eligable_ai[math.random(#eligable_ai)]
        local ai_id = redis.call('GET', ai_player .. ":display_name")
        table.insert(players, ai_id)
    end
end

local function check_for_ai_search(player)
    if tonumber(redis.call('GET', player .. ':matching')) == 1 then
        return false
    end

    if not redis.call('GET', player .. ':ai') then
        return false
    end

    local players = { player }
    local max_players = tonumber(redis.call('GET', player .. ':max_players'))

    fill_with_ai(players, max_players, redis.call('GET', player .. ':ai'))

    if #players == max_players then
        handle_match(redis.call('GET', player .. ':region'), players)
    end
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
        if tonumber(redis.call('INCR', player1 .. ':failed_searches')) > 10 then
            redis.call('SET', player1 .. ':fill_with_ai', 1)

            local all_fill_with_ai = true

            for _, player in ipairs(players) do
                if tonumber(redis.call('GET', player .. ':fill_with_ai')) == 0 then
                    all_fill_with_ai = false
                    break
                end
            end

            if all_fill_with_ai then
                fill_with_ai(players, max_player_count)

                if #players == max_player_count then
                    for _, player in ipairs(players) do
                        redis.call('SET', player .. ':ai', '*')
                    end

                    handle_match(region, players)
                end
            end
        end
    end
end

-- Check for hosts

searcher_keys = redis.call('KEYS', '*:host_requests')

local function check_for_start(searcher)
    if tonumber(redis.call('GET', searcher .. ':matching')) then
        return false
    end

    if tonumber(redis.call('GET', searcher .. ':start_requested')) == 0 then
        return false
    end

    redis.call('SET', searcher .. ':matching', 1)

    local player_keys = redis.call('KEYS', searcher .. ':joined_players:*')

    local players = {}

    for i = 1, #player_keys do
        local player_key = player_keys[i]
        local player = redis.call('GET', player_key)

        table.insert(players, player)
    end

    local region = redis.call('GET', searcher .. ':region')


    publish_new_match(region, players, redis.call('GET', searcher .. ':mode'), redis.call('GET', searcher .. ':game'), 0)

end


for i = 1, #searcher_keys do
    local searcher = searcher_keys[i]

    check_for_start(searcher)
end