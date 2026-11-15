local ArrayRotation = require( 'ArrayRotation' )

local foo = {
    { 1, 2, 3 },
    { 3, 4, 5 },
    { 6, 7, 8 },
}

local bar = {
    { 'a', 'b', 'c', 'd', 'e', 'f' },
    { 'g', 'h', 'i', 'j', 'k', 'l' },
    { 'm', 'n', 'o', 'p', 'q', 'r' },
}

local function dump( array )
    local str = ''
    for r = 1, #array do
        for c = 1, #array[r] do
            str = str .. ' ' .. array[r][c]
        end
        str = str .. '\n'
    end
    print( str )
end

dump( foo )
dump(ArrayRotation.rotate( foo, 1 ))
dump(ArrayRotation.rotate( foo, 2 ))
dump(ArrayRotation.rotate( foo, 3 ))

dump( bar )
dump(ArrayRotation.rotate( bar, 1 ))
dump(ArrayRotation.rotate( bar, 2 ))
dump(ArrayRotation.rotate( bar, 3 ))
