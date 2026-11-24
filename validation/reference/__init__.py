"""Independent reference implementation of the NEXUS FABRIC development families.

Nothing in this package is derived from the product's source code.  Every
pattern is obtained from a 3D parametrisation of the surface, developed by a
generic procedure (geodesic-curvature integration of a directrix, rulings laid
off as straight lines), with a triangulation unroller and an mpmath tier as
further cross-checks.  Reference patterns are always drawn AS SEEN FROM THE
OUTSIDE of the formed part (pattern normal = outward surface normal).
"""
