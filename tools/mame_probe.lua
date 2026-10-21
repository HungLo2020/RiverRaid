-- Offscreen reference observation only; never part of the Rust runtime.
local output = assert(os.getenv('RIVERRAID_PROBE_OUT'))
local mode = os.getenv('RIVERRAID_PROBE_MODE') or 'motion'
local out = assert(io.open(output .. '/ram.jsonl', 'w'))
local memory = manager.machine.devices[':maincpu'].spaces['program']
local controls = manager.machine.ioport.ports
local switches = controls[':SWB']
local joystick = controls[':joyport1:joy:JOY']
local frame, previous = 0, ''
emu.register_frame_done(function()
    frame = frame + 1
    if frame == 60 then switches.fields['Reset Game']:set_value(1) end
    if frame == 65 then switches.fields['Reset Game']:set_value(0) end
    if frame == 160 then joystick.fields['P1 Button 1']:set_value(1) end
    if frame == 165 then joystick.fields['P1 Button 1']:set_value(0) end
    if mode == 'motion' then
        if frame == 200 then joystick.fields['P1 Right']:set_value(1) end
        if frame == 216 then joystick.fields['P1 Right']:set_value(0); joystick.fields['P1 Left']:set_value(1) end
        if frame == 232 then joystick.fields['P1 Left']:set_value(0) end
        if frame == 240 then joystick.fields['P1 Up']:set_value(1) end
        if frame == 260 then joystick.fields['P1 Up']:set_value(0); joystick.fields['P1 Down']:set_value(1) end
        if frame == 280 then joystick.fields['P1 Down']:set_value(0) end
    end
    local signature = string.format('%02x%02x', memory:read_u8(0xe9), memory:read_u8(0xea))
    local selected = mode == 'motion' and frame >= 155 and frame <= 300
        or mode == 'generator' and frame >= 65 and signature ~= previous
    if selected then
        local bytes = {}
        for address = 0x80,0xff do bytes[#bytes+1] = string.format('%02x', memory:read_u8(address)) end
        out:write(string.format('{"frame":%d,"ram":"%s"}\n', frame, table.concat(bytes)))
        out:flush()
        previous = signature
    end
    if mode == 'generator' and frame >= 180 then
        -- Normal maximum speed only. Invulnerability/fuel instrumentation is
        -- separately identified in the derived trainer ROM, not hidden in RAM.
        memory:write_u8(0xb5,254)
    end
    if frame == 240 then manager.machine.screens[':screen']:snapshot(output .. '/reference.png') end
end)
