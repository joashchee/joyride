# Keyed Array

A small, pure Lua library that adds keyed arrays. Items in a keyed array can be referred to by their position in the array or an assigned key, similar to an ordered dictionary.

To use the class in your projects, just download keyedArray.lua and require it like this:

`local KeyedArray = require ("keyedArray")`

### Example Usage:

```lua
local KeyedArray = require ("keyedArray")
local KeyedArrayObj = KeyedArray:new ()

KeyedArrayObj:insert ("test1", 10)
KeyedArrayObj:insert ("test3", 30, 3)
KeyedArrayObj:insert ("test2", 20, 2)
KeyedArrayObj:insert ("test4", 40, 10)
KeyedArrayObj:insert ("test5", 50, 5)

KeyedArrayObj:delete (5, "array")

print ("SIZE:", KeyedArrayObj:size ())
print ("ITEM 3:", KeyedArrayObj:get (3, "array"))
print ("ITEM 3:", KeyedArrayObj:get ("test3", "key"))

print ("===ALL ITEMS===")
for pos, key, val in KeyedArrayObj:pairs () do
    print (pos, key, val)
end
```

### Documentation:

```lua
--- Creates a new KeyedArray object.
-- @return (table) A KeyedArray object
KeyedArray:new ()

--- Inserts a new item into the keyed array.
-- It will fail if an item already exists at the given key
-- @param key (table key) The key of the new item
-- @param value (any) The value of the new item
-- @param position (int) The position of the item in the array. Defaults to the current size of the array + 1 and will fix values outside the array bounds
-- @return (bool) True if the operation was successful, otherwise false
KeyedArray:insert (key, value, position)

--- Removes an item from the keyed array.
-- @raise When the provided keyType is invalid
-- @param key (table key) The key or position of the item
-- @param indexType(string) The type of key being used
KeyedArray:delete (key, indexType)

--- Gets the value of an item from the keyed array.
-- @raise When the provided keyType is invalid
-- @param key (table key) The key or position of the item
-- @param indexType(string) The type of key being used
KeyedArray:get (key, indexType)

--- Gets the current number of items inside the object.
-- @return (int) The number of items inside the object
KeyedArray:size ()

--- Gets the corresponding key or array index for a value.
-- @param key (table key) The key or position of the item
-- @param indexType (string) The type of index being used (either "array" or "key")
KeyedArray:getKey (key, indexType)

--- An iterator function for the keyed array object.
-- This will iterate over the items based on their order in the array
-- @usage for pos, key, value in KeyedArrayObj:pairs () do print (value) end
-- @return (function) An iterator function
KeyedArray:pairs ()
```
