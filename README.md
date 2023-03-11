# Stele
Stele is a Single Writer, Many Reader append-only concurrent data structure with support for no_std and the Allocator API

## How does it work?

Stele is, in essence, an array of exponentially larger arrays, where the nth element of the outer array holds 2<sup>n</sup> elements.
