import { Module } from '@nestjs/common';
import { AuthController } from './auth.controller';
import { AuthService } from './auth.service';
import { TokenRepository } from './token.repository';
import { UsersService } from '../users/users.service';

@Module({
  controllers: [AuthController],
  providers: [AuthService, TokenRepository, UsersService],
  exports: [AuthService],
})
export class AuthModule {}
