--!df flags=allow-undeclared-keys,disable-atomicity

-- Extended Matchmaking Capabilities
-- * Compatibility
-- * Tolerance

local function calculate_compatibility(player1, player2)
    local elo1 = tonumber(redis.call('GET', player1 .. ':elo'))
    local elo2 = tonumber(redis.call('GET', player2 .. ':elo'))
    local performance1 = tonumber(redis.call('GET', player1 .. ':performance')) or 0
    local performance2 = tonumber(redis.call('GET', player2 .. ':performance')) or 0
    local games1 = tonumber(redis.call('GET', player1 .. ':games')) or 1
    local games2 = tonumber(redis.call('GET', player2 .. ':games')) or 1

    local skill1 = elo1 + (elo1 * performance1 / games1)
    local skill2 = elo2 + (elo2 * performance2 / games2)

    return math.abs(skill1 - skill2)
end

local function is_group_compatible(players, wait_time, avg_players, strict_cycles, delta_s, epsilon)
    local total_compatibility = 0
    local n = #players

    for i = 1, n - 1 do
        total_compatibility = total_compatibility + calculate_compatibility(players[i], players[i + 1])
    end

    local avg_compatibility = total_compatibility / (n - 1)
    local tolerance = calculate_tolerance(wait_time, avg_players, strict_cycles, delta_s)

    return avg_compatibility - tolerance <= epsilon
end

local function calculate_tolerance(wait_time, avg_players, strict_cycles, delta_s)
    return delta_s / (1 + math.exp(-avg_players / (avg_players + strict_cycles^2) * (wait_time - strict_cycles)))
end

local function can_play_together(players)
    for i = 1, #players do
        local player = players[i]

        if tonumber(redis.call('EXISTS', player .. ':matching')) == 1 and tonumber(redis.call('GET', player .. ':matching')) == 1 then
            return false
        end

        if redis.call('EXISTS', player .. ':ai') == 1 and redis.call('GET', player .. ':ai') ~= '.' then
            return false
        end

        local player_elo = tonumber(redis.call('GET', player .. ':elo'))
        local player_mode = redis.call('GET', player .. ':mode')
        local player_game = redis.call('GET', player .. ':game')
        local player_region = redis.call('GET', player .. ':region')

        for j = i + 1, #players do
            local other = players[j]

            if tonumber(redis.call('EXISTS', other .. ':matching')) == 1 and tonumber(redis.call('GET', other .. ':matching')) == 1 then
                return false
            end

            if redis.call('EXISTS', other .. ':ai') == 1 and redis.call('GET', other .. ':ai') ~= '.' then
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
    publish_new_match(region, players, redis.call('GET', players[1] .. ':mode'), redis.call('GET', players[1] .. ':game'), redis.call('EXISTS', players[1] .. ':ai') == 1 and 1 or 0)
end

local function check_for_ai_search(player)
    if tonumber(redis.call('EXISTS', player .. ':matching')) == 1 and tonumber(redis.call('GET', player .. ':matching')) == 1 then
        return false
    end

    local ai = redis.call('GET', player .. ':ai')

    if not ai or ai == "." then
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

-- Try match everyone else
for i = 1, #searcher_keys do
    local player1 = searcher_keys[i]
    local wait_time = tonumber(redis.call('GET', player1 .. ':wait_time')) or 0
    local avg_players = tonumber(redis.call('GET', 'avg_players_24h')) or 20
    local strict_cycles = 10
    local delta_s = 500
    local epsilon = 50

    local players = { player1 }
    for j = i + 1, #searcher_keys do
        local player2 = searcher_keys[j]
        table.insert(players, player2)

        if can_play_together(players) and is_group_compatible(players, wait_time, avg_players, strict_cycles, delta_s, epsilon) then
            if #players == tonumber(redis.call('GET', player1 .. ':max_players')) then
                break
            end
        else
            table.remove(players, #players)
        end
    end

    local region = redis.call('GET', player1 .. ':region')
    if #players >= tonumber(redis.call('GET', player1 .. ':min_players')) then
        handle_match(region, players)
    else
        if tonumber(redis.call('INCR', player1 .. ':failed_searches')) > 10 then
            if redis.call('GET', player1 .. ':ai') ~= '.' then
                redis.call('SET', player1 .. ':fill_with_ai', 1)
            end

            local all_fill_with_ai = true

            for _, player in ipairs(players) do
                if tonumber(redis.call('EXISTS', player .. ':fill_with_ai')) == 0 or tonumber(redis.call('GET', player .. ':fill_with_ai')) ~= 1 then
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

