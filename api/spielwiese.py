from htpy import body, header, main, footer, section, nav
from htpy import article, dialog, button, ul, li, div, img
from htpy import form, input, label, textarea, select, option
from htpy import h1, h2, h3, h4, h5, h6, p, br, strong
from htpy import table, thead, tbody, tr, th, td


def navigation():
    _nav = nav(".navbar")
    _nav = _nav[ul[li["hello"]]]
    return _nav


print(navigation())
