import { Controller, Get, Patch, Delete } from '@nestjs/common';
import { UsersService } from './users.service';

@Controller('users')
export class UsersController {
  constructor(private readonly users: UsersService) {}
  @Get()
  list() { return this.users.list(); }
  // Same route, distinct declaration: must not overwrite the first endpoint.
  @Get()
  alias() { return ['alias']; }
  @Patch(':id')
  update() { return 'updated'; }
  @Delete(':id')
  remove() { return 'removed'; }
}
