// Dessine l'icône de Jauge (1024 px) : l'anneau de la barre des menus sur un carré graphite. swift scripts/icone.swift <sortie.png>
import AppKit

let taille: CGFloat = 1024
let image = NSImage(size: NSSize(width: taille, height: taille))
image.lockFocus()
let ctx = NSGraphicsContext.current!.cgContext

// Gabarit des icônes macOS : un carré arrondi de 824 px centré.
let carre = CGRect(x: 100, y: 100, width: 824, height: 824)
let forme = CGPath(roundedRect: carre, cornerWidth: 186, cornerHeight: 186, transform: nil)
ctx.addPath(forme)
ctx.clip()
let degrade = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(),
                         colors: [CGColor(red: 0.20, green: 0.20, blue: 0.23, alpha: 1), CGColor(red: 0.10, green: 0.10, blue: 0.12, alpha: 1)] as CFArray,
                         locations: [0, 1])!
ctx.drawLinearGradient(degrade, start: CGPoint(x: 0, y: 924), end: CGPoint(x: 0, y: 100), options: [])

let centre = CGPoint(x: taille / 2, y: taille / 2), rayon: CGFloat = 250
ctx.setLineWidth(84)
ctx.setLineCap(.round)
ctx.setStrokeColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.14))
ctx.addArc(center: centre, radius: rayon, startAngle: 0, endAngle: 2 * .pi, clockwise: false)
ctx.strokePath()
// L'arc part du haut et tourne dans le sens des aiguilles d'une montre, aux deux tiers.
ctx.setStrokeColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.95))
ctx.addArc(center: centre, radius: rayon, startAngle: .pi / 2, endAngle: .pi / 2 - 2 * .pi * 0.68, clockwise: true)
ctx.strokePath()
image.unlockFocus()

let rep = NSBitmapImageRep(data: image.tiffRepresentation!)!
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
