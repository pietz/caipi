from werkzeug.datastructures import ImmutableMultiDict

# Your ImmutableMultiDict
data = ImmutableMultiDict([
    ('name', 'Spellchecker'),
    ('instructions', 'Fix the spelling of the `input` text and return the corrected text. Set the `was_corrected` field accordingly, if fixes were necessary.'),
    ('req_name', 'input'),
    ('req_dtype', 'Text'),
    ('res_name', 'output'),
    ('res_name', 'was_corrected'),
    ('res_dtype', 'Text'),
    ('res_dtype', 'Text')
])

print(list(data.items(multi=True)))

# To iterate and get all values for each key:
for key in data:
    print(f"{key}: {data.getlist(key)}")
