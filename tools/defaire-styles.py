"""Un fichier a la fois : remplacer les styles en ligne par des classes.

Regle stricte : on ne remplace un `style="..."` que si **toute** sa valeur est
couverte par les classes connues. Un style qui porte aussi une largeur, un
`display` ou une marge inconnue est laisse tel quel — c'est lui qui casserait
une mise en page.

Usage : python tools/defaire-styles.py <fichier.rs> [--ecrire]
"""

import io
import re
import sys

# Valeur de style -> classes. Volontairement courte : chaque entree est une
# equivalence exacte, pas une approximation.
EQUIVALENCES = {
    "color:#78736c": "muet",
    "color:#78736c;font-size:13.5px": "aide",
    "color:#78736c;font-size:12.5px": "muet petite",
    "font-size:12.5px": "petite",
    "margin-top:10px": "haut",
    "width:100%": "pleine",
    "color:#a33a2a": "rouge",
}

chemin = sys.argv[1]
ecrire = "--ecrire" in sys.argv
texte = io.open(chemin, encoding="utf-8").read()

# Une balise ouvrable, son contenu, et l'attribut style eventuel.
motif = re.compile(r"<([a-zA-Z][\w-]*)([^<>]*?)\sstyle=\"([^\"]*)\"([^<>]*?)>")
remplaces, laisses = 0, []


def traiter(m):
    global remplaces
    balise, avant, valeur, apres = m.group(1), m.group(2), m.group(3), m.group(4)
    classes = EQUIVALENCES.get(valeur.strip())
    if not classes:
        laisses.append(valeur)
        return m.group(0)
    if "class=" in avant or "class=" in apres:
        # Deux attributs class sur la meme balise : on ne tente pas.
        laisses.append(valeur + " (class deja posee)")
        return m.group(0)
    remplaces += 1
    return f"<{balise}{avant} class=\"{classes}\"{apres}>"


nouveau = motif.sub(traiter, texte)
print(f"{chemin} : {remplaces} style(s) remplace(s), {len(laisses)} laisse(s)")
for v in sorted(set(laisses))[:8]:
    print(f"    laisse : style=\"{v}\"")
if ecrire and remplaces:
    io.open(chemin, "w", encoding="utf-8", newline="\n").write(nouveau)
    print("    ecrit")
