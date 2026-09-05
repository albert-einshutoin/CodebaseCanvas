import { Module, Controller, Get } from '@nestjs/common';
import { UsersService } from '../users/users.service';

export class MockUsersService {
  list() { return ['mock']; }
}
@Controller('override')
export class OverrideController {
  constructor(private readonly users: UsersService) {}
  @Get()
  list() { return this.users.list(); }
}
@Module({
  controllers: [OverrideController],
  providers: [{ provide: UsersService, useClass: MockUsersService }],
})
export class OverrideModule {}
