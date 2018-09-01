local function print_req_info(req)
    local host = "0.0.0.0"
    if req.host then
        host = req.host
    end

    local message = "Host: " .. host .. "\n" .. req.req_line .. "\n\nHTTP headers:\n"

    for k, v in pairs(req.headers) do
        message = message .. k .. ": " .. v .. "\n"
    end

    message = message .. "\nRequest body:\n" .. req.body

    print(message)
end

return {
    print_req_info = print_req_info
}